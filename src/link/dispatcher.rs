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
    information_rate::{byte_per_second, megabit_per_second},
    time::microsecond,
    u64::{Information as InformationU64, InformationRate as InformationRateU64},
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
    ) -> Result<()> {
        let core_id_mngr = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let core_id_drain = self.core_pool.as_mut().and_then(|cp| cp.pop());
        let mut core_id_trans = self.core_pool.as_mut().and_then(|cp| cp.pop());

        if let Some(core_id_mngr) = core_id_mngr {
            info!("Link {}: start dispatcher (Core: {}).", self.id, core_id_mngr.id);
            set_for_current(core_id_mngr);
        } else {
            info!("Link {}: start dispatcher.", self.id);
        }
        set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");

        let spin_sleep = SpinSleeper::default();
        let rdp = route_metric_queue.pop_next_route_metric().ok_or_eyre("Could not get initial RDP")?;
        let btldr: InformationRateU64 = rdp.btldr;
        let delay = rdp.delay;
        let mut route_id = rdp.route_id;

        let channel_size: Information = self.calculate_bottleneck_buffer_size(btldr, delay);
        let (sender, receiver) = byte_bounded_channel(InformationU64::new::<bit>(channel_size.get::<bit>() as u64));

        let drainer = Drainer::new(self.id, self.startup_mode, sender);
        let drainer = Arc::new(drainer);
        let ld_1 = drainer.clone();
        let ld = thread::spawn(move || {
            ld_1.run(socket_input, core_id_drain);
        });

        let mut active_pacer = Arc::new(Pacer::create(route_id, receiver, inflight_queue.clone(), btldr, delay));
        let at0 = active_pacer.clone();
        let mut t_h = thread::spawn(move || {
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
                debug!("We need to wait {} for next RDP.", wait_time.as_millis());
                spin_sleep.sleep_ns(wait_time.as_nanos().try_into().unwrap());
            } else {
                let rdp = route_metric_queue.pop_next_route_metric().ok_or_eyre("Could not get next RDP")?;
                let btldr: InformationRateU64 = rdp.btldr;
                let delay = rdp.delay;
                debug!(
                    "{}ms since start: running metric {}-{}",
                    time_since_start.as_millis(),
                    btldr.get::<megabit_per_second>(),
                    delay.as_millis()
                );
                let new_route_id = rdp.route_id;
                if route_id != new_route_id {
                    debug!("Switch route {}->{}, set delay to {}ms", route_id, new_route_id, delay.as_millis());
                    let channel_size: Information = self.calculate_bottleneck_buffer_size(btldr, delay);
                    let (sender, receiver) = byte_bounded_channel(InformationU64::new::<bit>(channel_size.get::<bit>() as u64));
                    drainer.set_tx(sender);
                    active_pacer.set_stopped();
                    // TODO: return core id after Pacer finished via JoinHandle?
                    if let Some(old_core_id_trans) = core_id_trans {
                        if let Some(cp) = self.core_pool.as_mut() {
                            cp.push(old_core_id_trans); // return old core id
                            core_id_trans = cp.pop();
                        }
                    }
                    active_pacer = Arc::new(Pacer::create(new_route_id, receiver, inflight_queue.clone(), btldr, delay));
                    let at0 = active_pacer.clone();
                    t_h = thread::spawn(move || {
                        at0.run(core_id_trans);
                    });
                    route_id = new_route_id;
                } else {
                    debug!(
                        "Update route, set delay={} ms and btldr={} Mbps",
                        delay.as_millis(),
                        btldr.get::<megabit_per_second>()
                    );
                    active_pacer.update_datarate(btldr);
                    active_pacer.update_delay(delay);
                }
            }
        }

        ld.join().unwrap();
        t_h.join().unwrap();

        Ok(())
    }

    fn calculate_bottleneck_buffer_size(&self, datarate: InformationRateU64, delay: Duration) -> Information {
        let bdp: Information = Self::calculate_bdp(datarate, delay);
        let buffer_size: Information = bdp * self.buffer_size_multiplier;
        debug!(
            "Link {}: Set BDP={}(kB), BtlBufferSize={}(kB)",
            self.id,
            bdp.get::<kilobyte>(),
            buffer_size.get::<kilobyte>()
        );
        buffer_size
    }

    fn time_since_start() -> Duration {
        Instant::now() - Runtime::access_app_start_time()
    }

    pub fn calculate_bdp(datarate: InformationRateU64, delay: Duration) -> Information {
        let rtt: Time = Time::new::<microsecond>(delay.as_micros() as f64 * 2.0);
        let datarate: InformationRate = InformationRate::new::<byte_per_second>(datarate.get::<byte_per_second>() as f64);
        (datarate * rtt).into()
    }
}
