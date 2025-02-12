use std::{
    sync::{Arc, Condvar, Mutex},
    thread,
    time::{Duration, Instant},
};

use core_affinity::{set_for_current, CoreId};
use eyre::{OptionExt, Result};
use log::{debug, info};
use pnet::datalink::DataLinkReceiver;
use spin_sleep::SpinSleeper;
use thread_priority::{set_current_thread_priority, ThreadPriority};
use uom::si::{
    f64::{Information, InformationRate, Time},
    information::{bit, kilobyte},
    information_rate::megabit_per_second,
    time::microsecond,
};

use crate::{
    byte_bounded_channel::byte_bounded_channel,
    cli::StartupMode,
    inflight_queue::InflightQueue,
    link::{drainer::Drainer, pacer::Pacer},
    route_metrics::RouteMetricQueue,
    runtime::Runtime,
};

pub struct Dispatcher {
    id: usize,
    startup_mode: StartupMode,
    buffer_size_multiplier: f64,
    core_pool: Option<Vec<CoreId>>,
}

impl Dispatcher {
    pub fn new(id: usize, startup_mode: StartupMode, buffer_size_multiplier: f64, core_pool: Option<Vec<CoreId>>) -> Dispatcher {
        Dispatcher {
            id,
            startup_mode,
            buffer_size_multiplier,
            core_pool,
        }
    }

    pub fn run(
        &mut self,
        socket_input: Box<dyn DataLinkReceiver>,
        mut route_metric_queue: RouteMetricQueue,
        inflight_queue: Arc<(Mutex<InflightQueue>, Condvar)>,
        reconfiguration_delay: Duration,
    ) -> Result<()> {
        let core_id_mngr = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let core_id_drain = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let core_id_trans = self.core_pool.as_mut().and_then(|cp| cp.pop());

        if let Some(core_id_mngr) = core_id_mngr {
            info!("Link {}: start dispatcher (Core: {}).", self.id, core_id_mngr.id);
            set_for_current(core_id_mngr);
        } else {
            info!("Link {}: start dispatcher.", self.id);
        }
        set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");

        let spin_sleep = SpinSleeper::default();
        let rdp = route_metric_queue.pop_next_route_metric().ok_or_eyre("Could not get initial RDP")?;
        let btldr: InformationRate = rdp.btldr;
        let delay = rdp.delay;
        let mut route_id = rdp.route_id;

        // create ByteBoundedChannel
        let channel_size: Information = self.calculate_bottleneck_buffer_size(btldr, delay);
        let (sender, receiver, channel_handle) = byte_bounded_channel(Information::new::<bit>(channel_size.get::<bit>()));

        // create drainer
        let drainer = Drainer::new(self.id, self.startup_mode, sender);
        let drainer = Arc::new(drainer);
        let ld_1 = drainer.clone();
        let ld = thread::spawn(move || {
            ld_1.run(socket_input, core_id_drain);
        });

        // create pacer
        let pacer = Arc::new(Pacer::create(route_id, receiver, inflight_queue.clone(), btldr, delay));
        let at0 = pacer.clone();
        let t_h = thread::spawn(move || {
            at0.run(core_id_trans);
        });

        loop {
            if !Runtime::is_app_start_time_initialized() {
                // info!("Waiting for first packet");
                spin_sleep.sleep_ns(10_000);
                continue;
            }

            let time_since_start = Dispatcher::time_since_start();
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
                    pacer.switch_route(new_route_id, reconfiguration_delay);
                    let new_channel_size: Information = self.calculate_bottleneck_buffer_size(btldr, delay);
                    channel_handle.update_capacity(new_channel_size);
                }
            }
        }
        ld.join().unwrap();
        t_h.join().unwrap();

        Ok(())
    }

    fn calculate_bottleneck_buffer_size(&self, datarate: InformationRate, delay: Duration) -> Information {
        let bdp: Information = Self::calculate_bdp(datarate, delay);
        let buffer_size: Information = bdp * self.buffer_size_multiplier;
        debug!(
            "Link {}: Set BDP={} kB, BtlBufferSize={} kB",
            self.id,
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
