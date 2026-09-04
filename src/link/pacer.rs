use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};

use core_affinity::{set_for_current, CoreId};
use crossbeam::atomic::AtomicCell;
use log::debug;
use spin_sleep::SpinSleeper;
use thread_priority::{set_current_thread_priority, ThreadPriority};
use uom::si::{
    f64::{InformationRate, Time},
    information_rate::{byte_per_second, megabit_per_second},
    time::{nanosecond, second},
};

use crate::{
    byte_bounded_channel::ByteReceiver,
    inflight_queue::{InflightQueue, ScheduleResult},
};

struct DataRateCell {
    dr: AtomicU64,
}

impl DataRateCell {
    fn new(initial_btldr: InformationRate) -> Self {
        Self {
            dr: AtomicU64::new(initial_btldr.get::<byte_per_second>() as u64),
        }
    }

    fn load(&self) -> InformationRate {
        InformationRate::new::<byte_per_second>(self.dr.load(Ordering::Relaxed) as f64)
    }

    fn store(&self, v: InformationRate) {
        self.dr.store(v.get::<byte_per_second>() as u64, Ordering::Relaxed);
    }
}

pub struct Pacer {
    route_id: AtomicU64,
    buffer: ByteReceiver,
    link: Arc<(Mutex<InflightQueue>, Condvar)>,
    btldr: DataRateCell,
    delay: AtomicU64,
    reconfigure_until: AtomicCell<Option<Instant>>,
    max_thread_priority: bool,
}

impl Pacer {
    pub fn create(
        route_id: u64,
        buffer: ByteReceiver,
        link: Arc<(Mutex<InflightQueue>, Condvar)>,
        initial_btldr: InformationRate,
        initial_delay: Duration,
        max_thread_priority: bool,
    ) -> Self {
        Self {
            route_id: AtomicU64::new(route_id),
            buffer,
            link,
            btldr: DataRateCell::new(initial_btldr),
            delay: AtomicU64::new(initial_delay.as_millis().try_into().unwrap()),
            reconfigure_until: AtomicCell::new(None),
            max_thread_priority,
        }
    }

    pub fn run(&self, core_id: Option<CoreId>) {
        if let Some(core_id) = core_id {
            set_for_current(core_id);
        }
        if self.max_thread_priority {
            set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");
        }
        let spin_sleep = SpinSleeper::default();

        let mut last_packet = Instant::now(); // stores time when last packet was sent
        const DURATION: Duration = Duration::from_secs(2);
        loop {
            while let Ok(data) = self.buffer.recv_timeout(DURATION) {
                if let Some(until) = self.reconfigure_until.take() {
                    let remaining = until - Instant::now();
                    spin_sleep.sleep_ns(remaining.as_nanos() as u64);
                }

                // simulate datarate, taking into account the time code execution takes
                let length_byte: f64 = data.len() as f64;
                let btldr_byte_per_s = self.btldr.load().get::<byte_per_second>();
                let transmission_time: Time = Time::new::<second>(length_byte / btldr_byte_per_s);
                let transmission_time_ns = transmission_time.get::<nanosecond>() as u64;
                let time_elapsed_since_last = last_packet.elapsed().as_nanos() as u64;
                let sleep_dur = transmission_time_ns.saturating_sub(time_elapsed_since_last); // ensure sleep_dur >= 0
                spin_sleep.sleep_ns(sleep_dur);

                let (lock, cvar) = &*self.link;
                let mut pq = lock.lock().unwrap();
                match pq
                    .schedule_packet(data, self.route_id.load(Ordering::Relaxed), self.delay.load(Ordering::Relaxed))
                    .0
                {
                    ScheduleResult::Changed => {
                        cvar.notify_all();
                    }
                    ScheduleResult::Unchanged => {}
                }
                last_packet = Instant::now(); // update last sent time
            }
        }
    }

    pub fn switch_route(&self, new_route_id: u64, reconfiguration_delay: Duration) {
        debug!("Update route_id to {}", new_route_id);
        self.route_id.store(new_route_id, Ordering::Relaxed);
        self.reconfigure_until.store(Some(Instant::now() + reconfiguration_delay));
    }

    pub fn update_datarate(&self, new_btldr: InformationRate) {
        debug!("Update btldr to {} Mbps", new_btldr.get::<megabit_per_second>());
        self.btldr.store(new_btldr);
    }

    pub fn update_delay(&self, new_delay: Duration) {
        let delay_ms: u64 = new_delay.as_millis().try_into().unwrap();
        debug!("Update delay to {}ms", delay_ms);
        self.delay.store(delay_ms, Ordering::Relaxed);
    }
}
