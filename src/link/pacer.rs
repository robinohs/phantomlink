use std::{
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};

use core_affinity::{set_for_current, CoreId};
use log::{debug, info};
use spin_sleep::SpinSleeper;
use thread_priority::{set_current_thread_priority, ThreadPriority};
use uom::si::{
    f64::Time,
    information_rate::{byte_per_second, megabit_per_second},
    time::{nanosecond, second},
    u64::InformationRate,
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
            dr: AtomicU64::new(initial_btldr.get::<byte_per_second>()),
        }
    }

    fn load(&self) -> InformationRate {
        InformationRate::new::<byte_per_second>(self.dr.load(Ordering::Relaxed))
    }

    fn store(&self, v: InformationRate) {
        self.dr.store(v.get::<byte_per_second>(), Ordering::Relaxed);
    }
}

pub struct Pacer {
    route_id: u64,
    buffer: ByteReceiver,
    link: Arc<(Mutex<InflightQueue>, Condvar)>,
    btldr: DataRateCell,
    delay: AtomicU64,
    stopped: AtomicBool,
}

impl Pacer {
    pub fn create(
        route_id: u64,
        buffer: ByteReceiver,
        link: Arc<(Mutex<InflightQueue>, Condvar)>,
        initial_btldr: InformationRate,
        initial_delay: Duration,
    ) -> Self {
        Self {
            route_id,
            buffer,
            link,
            btldr: DataRateCell::new(initial_btldr),
            delay: AtomicU64::new(initial_delay.as_millis().try_into().unwrap()),
            stopped: AtomicBool::new(false),
        }
    }

    pub fn run(&self, core_id: Option<CoreId>) {
        if let Some(core_id) = core_id {
            set_for_current(core_id);
        }
        set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");
        let spin_sleep = SpinSleeper::default();

        let mut last_packet = Instant::now(); // stores time when last packet was sent
        const DURATION: Duration = Duration::from_secs(2);
        loop {
            if self.stopped.load(Ordering::Relaxed) && self.buffer.is_empty() {
                break;
            }
            while let Ok(data) = self.buffer.recv_timeout(DURATION) {
                // simulate datarate, taking into account the time code execution takes
                let length_byte: f64 = data.len() as f64;
                let btldr_byte_per_s = self.btldr.load().get::<byte_per_second>() as f64;
                let transmission_time: Time = Time::new::<second>(length_byte / btldr_byte_per_s);
                let transmission_time_ns = transmission_time.get::<nanosecond>() as u64;
                let time_elapsed_since_last = last_packet.elapsed().as_nanos() as u64;
                let sleep_dur = transmission_time_ns.saturating_sub(time_elapsed_since_last); // ensure sleep_dur >= 0
                spin_sleep.sleep_ns(sleep_dur);

                let (lock, cvar) = &*self.link;
                let mut pq = lock.lock().unwrap();
                match pq.try_schedule_packet(data, self.route_id, self.delay.load(Ordering::Relaxed)).0 {
                    ScheduleResult::Changed => {
                        cvar.notify_all();
                    }
                    ScheduleResult::Unchanged => {}
                }
                last_packet = Instant::now(); // update last sent time
            }
        }

        info!("Exit pacer");
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

    pub fn set_stopped(&self) {
        info!("Stop pacer");
        self.stopped.store(true, Ordering::Relaxed);
    }
}
