use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

use core_affinity::{set_for_current, CoreId};
use crossbeam::atomic::AtomicCell;
use eyre::{OptionExt, Result};
use log::{debug, info};
use pnet::datalink::{DataLinkReceiver, DataLinkSender};
use spin_sleep::SpinSleeper;
use thread_priority::{set_current_thread_priority, ThreadPriority};
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::{bit, kilobyte},
    information_rate::megabit_per_second,
    time::microsecond,
};

use crate::{
    byte_bounded_channel::byte_bounded_channel, cli::StartupMode, inflight_queue::InflightQueue, route_metrics::RouteMetricQueue,
    runtime::Runtime, ReconfigurationMode,
};

use super::{deliverer::Deliverer, drainer::Drainer, pacer::Pacer};

pub struct OnewayVirtualLink {
    link_id: usize,
    startup_mode: StartupMode,
    buffer_size_multiplier: f64,
    core_pool: Option<Vec<CoreId>>,
}

impl OnewayVirtualLink {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        link_id: usize,
        startup_mode: StartupMode,
        buffer_size_multiplier: f64,
        core_pool: Option<Vec<CoreId>>,
    ) -> OnewayVirtualLink {
        OnewayVirtualLink {
            link_id,
            startup_mode,
            buffer_size_multiplier,
            core_pool,
        }
    }

    pub fn run(
        &mut self,
        input: Box<dyn DataLinkReceiver>,
        output: Box<dyn DataLinkSender>,
        mut route_metric_queue: RouteMetricQueue,
        reconfiguration_delay: Duration,
        reconfiguration_mode: ReconfigurationMode,
    ) -> Result<()> {
        let core_id_ovl = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let core_id_deliverer = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let core_id_drainer = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let core_id_pacer = self.core_pool.as_mut().and_then(|cp| cp.pop());

        if let Some(core_id_ovl) = core_id_ovl {
            info!("Link {}: start one-way virtual link (Core: {}).", self.link_id, core_id_ovl.id);
            set_for_current(core_id_ovl);
        } else {
            info!("Link {}: start one-way virtual link.", self.link_id);
        }
        set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");

        // create packet stacks for this link
        let spin_sleep = SpinSleeper::default();
        let rdp = route_metric_queue.pop_next_route_metric().ok_or_eyre("Could not get initial RDP")?;
        let btldr: InformationRate = rdp.btldr;
        let delay = rdp.delay; // initial delay
        let mut route_id = rdp.route_id;
        let inflight_queue = Arc::new((Mutex::new(InflightQueue::new(self.link_id, delay)), Condvar::new()));

        // create ByteBoundedChannel
        let channel_size: Information = self.calculate_bottleneck_buffer_size(btldr, delay);
        let (sender, receiver, channel_handle) = byte_bounded_channel(Information::new::<bit>(channel_size.get::<bit>()));

        // create & start drainer
        let drainer = Arc::new(Drainer::new(self.link_id, self.startup_mode, sender));
        let drainer_clone = drainer.clone();
        let thread_drainer = thread::spawn(move || {
            drainer_clone.run(input, core_id_drainer);
        });

        // create & start pacer
        let pacer = Arc::new(Pacer::create(route_id, receiver, inflight_queue.clone(), btldr, delay));
        let pacer_clone: Arc<Pacer> = pacer.clone();
        let thread_pacer = thread::spawn(move || {
            pacer_clone.run(core_id_pacer);
        });

        // create & start sender
        let inflight_queue_sender = inflight_queue.clone();
        let deliverer_reconfig_until = Arc::new(AtomicCell::new(None));
        let mut deliverer = Deliverer::new(self.link_id, inflight_queue_sender, deliverer_reconfig_until.clone());
        let thread_sender = thread::spawn(move || {
            deliverer.run(output, core_id_deliverer);
        });

        loop {
            if !Runtime::is_app_start_time_initialized() {
                // info!("Waiting for first packet");
                spin_sleep.sleep_ns(10_000);
                continue;
            }

            let time_since_start = OnewayVirtualLink::time_since_start();
            let next_route_metric = match route_metric_queue.peek_next_route_metric() {
                Some(rm) => rm,
                None => {
                    debug!("Found last route metric");
                    break;
                }
            };

            // if we still need to wait for next event
            if next_route_metric.time_after_start > time_since_start {
                let wait_time = next_route_metric.time_after_start - time_since_start;
                debug!("Wait {}ms for next route metric.", wait_time.as_millis());
                spin_sleep.sleep_ns(wait_time.as_nanos().try_into().unwrap());
            } else {
                let route_metric = route_metric_queue.pop_next_route_metric().ok_or_eyre("Could not get next RDP")?;
                let btldr: InformationRate = route_metric.btldr;
                let delay = route_metric.delay;
                debug!(
                    "{}ms since start: running metric {}-{}",
                    time_since_start.as_millis(),
                    btldr.get::<megabit_per_second>().round(),
                    delay.as_millis()
                );
                debug!(
                    "Update route, set delay={} ms and btldr={} Mbps",
                    delay.as_millis(),
                    btldr.get::<megabit_per_second>().round()
                );
                pacer.update_datarate(btldr);
                pacer.update_delay(delay);

                let new_route_id = route_metric.route_id;
                if route_id != new_route_id {
                    debug!("Switch route {}->{}", route_id, new_route_id);
                    route_id = new_route_id;

                    // GSL (Pacer: Ground - Satellite)
                    pacer.switch_route(new_route_id, reconfiguration_delay);
                    // ISL (Inflight Queue)
                    if let ReconfigurationMode::All = reconfiguration_mode {
                        let mut iq = inflight_queue.0.lock().unwrap();
                        iq.switch_route(reconfiguration_delay);
                    }
                    // GSL (Deliverer: Satellite - Ground)
                    deliverer_reconfig_until.store(Some(Instant::now() + reconfiguration_delay));

                    let new_channel_size: Information = self.calculate_bottleneck_buffer_size(btldr, delay);
                    channel_handle.update_capacity(new_channel_size);
                }
            }
        }

        thread_drainer.join().unwrap();
        thread_pacer.join().unwrap();
        thread_sender.join().unwrap();
        Ok(())
    }

    fn calculate_bottleneck_buffer_size(&self, datarate: InformationRate, delay: Duration) -> Information {
        let bdp: Information = Self::calculate_bdp(datarate, delay);
        let buffer_size: Information = bdp * self.buffer_size_multiplier;
        debug!(
            "Link {}: Set BDP={} kB, BtlBufferSize={} kB",
            self.link_id,
            bdp.get::<kilobyte>().round(),
            buffer_size.get::<kilobyte>().round()
        );
        buffer_size
    }

    fn time_since_start() -> Duration {
        Instant::now() - Runtime::access_app_start_time()
    }

    pub fn calculate_bdp(datarate: InformationRate, delay: Duration) -> Information {
        let rtt: Time = Time::new::<microsecond>(delay.as_micros() as f64 * 2.0);
        (datarate * rtt).into()
    }
}
