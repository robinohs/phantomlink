use log::{info, trace};

use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    hash::{Hash, Hasher},
    time::{Duration, Instant},
};

pub struct InflightQueue {
    link_id: usize,
    next_seq_id: u128,
    packets: BinaryHeap<Reverse<Packet>>,
}

pub enum GetResult {
    Packet(Packet),
    Next(Instant),
    None,
}

pub enum ScheduleResult {
    Changed,
    Unchanged,
}

impl InflightQueue {
    /// Creates a new InflightQueue instance with an empty priority queue for packets.
    pub fn new(link_id: usize, delay: Duration) -> InflightQueue {
        info!("Link {}: init inflight_queue with delay={}ms", link_id, delay.as_millis());
        InflightQueue {
            link_id,
            next_seq_id: 0,
            packets: BinaryHeap::with_capacity(100_000),
        }
    }

    /// Adds a packet to the packet queue by creating an packet that stores the packet content and the Instant after that the packet is ready to leave the queue.
    pub fn schedule_packet(&mut self, packet: Vec<u8>, route_id: u64, delay: u64) -> (ScheduleResult, u128) {
        let time = Instant::now();
        let tmp_id = self.next_seq_id;
        self.next_seq_id += 1;
        let exec_time = time + Duration::from_millis(delay);
        let ev = Packet {
            time: exec_time,
            seq_id: tmp_id,
            packet,
            route_id,
        };
        let ev = Reverse(ev);

        let current_max = self.packets.peek();
        let result = match current_max {
            Some(current_max) => match current_max.cmp(&ev) {
                std::cmp::Ordering::Greater | std::cmp::Ordering::Equal => ScheduleResult::Unchanged,
                std::cmp::Ordering::Less => ScheduleResult::Changed,
            },
            None => ScheduleResult::Changed,
        };
        self.packets.push(ev);
        trace!("Link: {}: schedule packet({}) with delay={}ms.", self.link_id, tmp_id, delay);
        (result, tmp_id)
    }

    /// Returns a packet which Instant is now or in the past.
    pub fn try_get_packet(&mut self) -> GetResult {
        if let Some(packet) = self.packets.peek() {
            let time = Instant::now();
            if time >= packet.0.time {
                GetResult::Packet(self.packets.pop().unwrap().0)
            } else {
                GetResult::Next(packet.0.time)
            }
        } else {
            GetResult::None
        }
    }
}

#[derive(Debug)]
pub struct Packet {
    pub seq_id: u128,
    pub time: Instant,
    pub packet: Vec<u8>,
    pub route_id: u64,
}

impl Hash for Packet {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.seq_id.hash(state);
    }
}

impl PartialEq for Packet {
    fn eq(&self, other: &Self) -> bool {
        self.seq_id == other.seq_id
    }
}

impl Eq for Packet {}

impl PartialOrd for Packet {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Packet {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if self.route_id == other.route_id {
            // if on same route, cmp by seq id
            self.seq_id.cmp(&other.seq_id)
        } else {
            // if not on same route, cmp by time
            self.time.cmp(&other.time)
        }
    }
}

#[cfg(test)]
pub mod test {
    use core::panic;
    use std::{
        thread,
        time::{Duration, Instant},
    };

    use super::{GetResult, InflightQueue, ScheduleResult};

    const BYTES: [u8; 5] = [5u8, 2u8, 7u8, 2u8, 4u8];

    fn default_queue() -> InflightQueue {
        InflightQueue::new(0, Duration::from_millis(50))
    }

    fn instant_eq(a: Instant, b: Instant) -> bool {
        let diff = if b > a { b - a } else { a - b };
        diff < Duration::from_micros(100)
    }

    #[test]
    fn test_instant_eq() {
        let now = Instant::now();
        assert!(instant_eq(now, now + Duration::from_micros(1)));
        assert!(!instant_eq(now, now + Duration::from_millis(1)));
        assert!(!instant_eq(now, now + Duration::from_secs(1)));
    }

    #[test]
    fn test_add_pop_packet() {
        let mut evq = default_queue();

        const DELAY: u64 = 50;
        let now = std::time::Instant::now();
        let target = now + Duration::from_millis(DELAY);
        match evq.schedule_packet(BYTES.to_vec(), 0, DELAY).0 {
            ScheduleResult::Changed => {}
            ScheduleResult::Unchanged => panic!("Insert into empty queue should change head."),
        }

        let packet = evq.try_get_packet();
        match packet {
            GetResult::Packet(_) => panic!("This packet is delay and should not be received."),
            GetResult::Next(instant) => assert!(instant_eq(instant, target)),
            GetResult::None => panic!("There is now a packet in the queue."),
        }
    }

    #[test]
    fn test_add_wait_pop_packet() {
        let mut evq = default_queue();

        const DELAY: u64 = 50;
        let id_first = match evq.schedule_packet(BYTES.to_vec(), 0, DELAY) {
            (ScheduleResult::Changed, id) => id,
            _ => panic!("Insert into empty queue should change head."),
        };
        thread::sleep(Duration::from_millis(DELAY + 2));

        let packet = evq.try_get_packet();
        match packet {
            GetResult::Packet(p) => {
                assert_eq!(p.seq_id, id_first)
            }
            _ => panic!("Packet should be ready."),
        }
    }

    #[test]
    fn test_packet_correct_order() {
        let mut evq = default_queue();

        const DELAY: u64 = 50;
        let id_first = match evq.schedule_packet(BYTES.to_vec(), 1, DELAY) {
            (ScheduleResult::Changed, id) => id,
            _ => panic!("Insert into empty queue should change head."),
        };
        let id_second = match evq.schedule_packet(BYTES.to_vec(), 1, DELAY) {
            (ScheduleResult::Unchanged, id) => id,
            _ => panic!("Second packet should arrive later."),
        };
        thread::sleep(Duration::from_millis(DELAY + 2));

        match evq.try_get_packet() {
            GetResult::Packet(p) => {
                assert_eq!(p.seq_id, id_first)
            }
            GetResult::Next(_) => panic!("Packet should be ready."),
            GetResult::None => panic!("There is now a packet in the queue."),
        };

        match evq.try_get_packet() {
            GetResult::Packet(p) => {
                assert_eq!(p.seq_id, id_second)
            }
            GetResult::Next(_) => panic!("Packet should be ready."),
            GetResult::None => panic!("There is now a packet in the queue."),
        };
    }

    #[test]
    fn test_packet_test_correct_order_faster_but_same_route() {
        let mut evq = default_queue();

        let now = Instant::now();
        const DELAY: u64 = 50;
        let target = now + Duration::from_millis(DELAY);

        let id_first = match evq.schedule_packet(BYTES.to_vec(), 0, DELAY) {
            (ScheduleResult::Changed, id) => id,
            _ => panic!("Insert into empty queue should change minimum."),
        };
        let id_second = match evq.schedule_packet(BYTES.to_vec(), 0, 25) {
            (ScheduleResult::Unchanged, id) => id,
            _ => panic!("Insert packet on same route should not change minimum."),
        };
        thread::sleep(Duration::from_millis(26));

        // check second packet did not overtake first
        match evq.try_get_packet() {
            GetResult::Next(instant) => assert!(instant_eq(instant, target)),
            _ => panic!("Should have a next packet."),
        };

        // check that after first would be ready, both are ready, and still in inserted order
        thread::sleep(Duration::from_millis(26));
        match evq.try_get_packet() {
            GetResult::Packet(p) => {
                assert_eq!(p.seq_id, id_first)
            }
            _ => panic!("Packet should be ready."),
        };

        match evq.try_get_packet() {
            GetResult::Packet(p) => {
                assert_eq!(p.seq_id, id_second)
            }
            _ => panic!("Packet should be ready."),
        };
    }

    #[test]
    fn test_packet_overtaking() {
        let mut evq = default_queue();

        let now = Instant::now();
        const DELAY: u64 = 50;
        let target = now + Duration::from_millis(DELAY);
        let id_first = match evq.schedule_packet(BYTES.to_vec(), 0, DELAY) {
            (ScheduleResult::Changed, id) => id,
            _ => panic!("Insert into empty queue should change head."),
        };
        let id_second = match evq.schedule_packet(BYTES.to_vec(), 1, 25) {
            (ScheduleResult::Changed, id) => id,
            _ => panic!("Faster packet on different route should overtake."),
        };
        thread::sleep(Duration::from_millis(26));

        // check if second packet has overtaken first in channel, and first is still on its way
        match evq.try_get_packet() {
            GetResult::Packet(packet) => {
                assert_eq!(packet.seq_id, id_second)
            }
            _ => panic!("Should get second packet."),
        }
        match evq.try_get_packet() {
            GetResult::Next(next) => {
                assert!(instant_eq(next, target))
            }
            _ => panic!("Should not get the first packet."),
        }

        // wait until second packet moved through channel
        thread::sleep(Duration::from_millis(26));
        match evq.try_get_packet() {
            GetResult::Packet(packet) => {
                assert_eq!(packet.seq_id, id_first)
            }
            _ => panic!("Should get first packet."),
        };
    }
}
