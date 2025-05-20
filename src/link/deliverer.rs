use std::sync::{Arc, Condvar, Mutex};

use core_affinity::{set_for_current, CoreId};
use crossbeam::atomic::AtomicCell;
use log::info;
use pnet::datalink::DataLinkSender;
use spin_sleep::SpinSleeper;
use std::time::Instant;
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::inflight_queue::{GetResult, InflightQueue};

pub struct Deliverer {
    id: usize,
    inflight_queue: Arc<(Mutex<InflightQueue>, Condvar)>,
    reconfigure_until: Arc<AtomicCell<Option<Instant>>>,
}

impl Deliverer {
    /// Creates a new ```Deliverer``` instance.
    pub fn new(
        id: usize,
        inflight_queue: Arc<(Mutex<InflightQueue>, Condvar)>,
        reconfigure_until: Arc<AtomicCell<Option<Instant>>>,
    ) -> Deliverer {
        Deliverer {
            id,
            inflight_queue,
            reconfigure_until,
        }
    }

    /// Starts the loop to check the two ```PacketStack``` and send those ```Packet``` with fulfilled send condition.
    pub fn run(&mut self, mut socket_output: Box<dyn DataLinkSender>, core_id: Option<CoreId>) {
        if let Some(core_id) = core_id {
            info!("Link {}: start sending on socket (Core: {}).", self.id, core_id.id);
            set_for_current(core_id);
        } else {
            info!("Link {}: start sending on socket.", self.id);
        }
        set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");
        let spin_sleep = SpinSleeper::default();

        let (lock, cvar) = &*self.inflight_queue;

        // Outer: Forward One Packet
        loop {
            let mut eq = lock.lock().unwrap();

            // Inner: Get One Packet
            loop {
                if let Some(until) = self.reconfigure_until.take() {
                    let remaining = until - Instant::now();
                    spin_sleep.sleep_ns(remaining.as_nanos() as u64);
                }

                match eq.try_get_packet() {
                    GetResult::Packet(pkt) => {
                        socket_output.send_to(&pkt.packet, None);
                        break;
                    }
                    GetResult::Next(instant) => {
                        let (eqq, _) = cvar.wait_timeout(eq, instant - Instant::now()).unwrap();
                        eq = eqq;
                        continue;
                    }
                    GetResult::None => {
                        eq = cvar.wait(eq).unwrap();
                        continue;
                    }
                }
            }
        }
    }
}
