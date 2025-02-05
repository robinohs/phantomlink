use std::sync::atomic::{AtomicU64, Ordering};
use std::{sync::Arc, time::Duration};

use crossbeam::channel::{unbounded, Receiver, Sender};
use log::debug;
use thiserror::Error;
use uom::si::{information::byte, u64::Information};

type Bytes = Vec<u8>;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum TrySendError {
    #[error("channel is full")]
    ChannelFullError,
    #[error("channel is disconnected")]
    ChannelDisconnected,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum RecvTimeoutError {
    #[error("recv timeout")]
    Timeout,
    #[error("channel is disconnected")]
    ChannelDisconnected,
}

pub struct ByteCell {
    len: AtomicU64,
}

impl ByteCell {
    fn new() -> Self {
        Self { len: AtomicU64::new(0) }
    }

    fn load(&self) -> Information {
        Information::new::<byte>(self.len.load(Ordering::Relaxed))
    }

    fn add(&self, v: Information) {
        self.len.fetch_add(v.get::<byte>(), Ordering::Relaxed);
    }

    fn sub(&self, v: Information) {
        self.len.fetch_sub(v.get::<byte>(), Ordering::Relaxed);
    }
}

pub struct ByteSender {
    len: Arc<ByteCell>,
    capacity: Information,
    sender: Sender<Bytes>,
}

impl ByteSender {
    pub fn try_send(&self, data: Bytes) -> Result<(), TrySendError> {
        let data_len: Information = Information::new::<byte>(data.len() as u64);

        let channel_len = self.len.load();
        let new_len = channel_len + data_len;
        // info!("Check {} against {}", new_size.get::<byte>(), self.capacity.get::<byte>());
        if new_len > self.capacity {
            debug!("ByteBoundedChannel: new size exceeds capacity. Packet is not accepted.");
            return Err(TrySendError::ChannelFullError);
        }
        self.len.add(data_len);

        if let Err(e) = self.sender.try_send(data) {
            let e = match e {
                crossbeam::channel::TrySendError::Full(_) => unreachable!("Unbounded channel was full"),
                crossbeam::channel::TrySendError::Disconnected(_) => TrySendError::ChannelDisconnected,
            };
            return Err(e);
        }
        Ok(())
    }
}

pub struct ByteReceiver {
    len: Arc<ByteCell>,
    receiver: Receiver<Bytes>,
}

impl ByteReceiver {
    pub fn recv_timeout(&self, timeout: Duration) -> Result<Bytes, RecvTimeoutError> {
        let data = match self.receiver.recv_timeout(timeout) {
            Ok(data) => Ok(data),
            Err(e) => match e {
                crossbeam::channel::RecvTimeoutError::Timeout => Err(RecvTimeoutError::Timeout),
                crossbeam::channel::RecvTimeoutError::Disconnected => Err(RecvTimeoutError::ChannelDisconnected),
            },
        }?;
        let data_size: Information = Information::new::<byte>(data.len() as u64);
        self.len.sub(data_size);
        Ok(data)
    }

    pub fn is_empty(&self) -> bool {
        self.receiver.is_empty()
    }
}

/// Creates a channel that is bounded to hold only a specific number of bytes calculated from the sum of an arbitrary number of messages.
/// Uses an unbounded cross_beam channel underneath.
pub fn byte_bounded_channel(capacity: Information) -> (ByteSender, ByteReceiver) {
    let (sender, receiver) = unbounded::<Bytes>();
    let len = Arc::new(ByteCell::new());
    let sender = ByteSender {
        sender,
        len: len.clone(),
        capacity,
    };
    let receiver = ByteReceiver {
        receiver,
        len: len.clone(),
    };
    (sender, receiver)
}

#[cfg(test)]
pub mod test {
    use crossbeam::atomic::AtomicCell;
    use uom::si::information::kilobyte;

    use super::*;

    #[test]
    fn test_lock_free() {
        assert_eq!(AtomicCell::<u64>::is_lock_free(), true);
    }

    #[test]
    /// tests if max_capacity can be used
    fn test_capacity_enough_space() {
        let (sender, _receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data = vec![0; 1000];
        sender.try_send(data).unwrap();
    }

    #[test]
    /// tests if multiple packets can be send if there size is below max_capacity
    fn test_capacity_enough_space_two_packets() {
        let (sender, _receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data = vec![0; 500];
        sender.try_send(data).unwrap();
        let data = vec![0; 500];
        sender.try_send(data).unwrap();
    }

    #[test]
    /// tests if packets are not accepted if they exceed max_capacity
    fn test_capacity_exceeding() {
        let (sender, _receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data = vec![0; 1001];
        let err = sender.try_send(data).unwrap_err();
        assert_eq!(err, TrySendError::ChannelFullError);
    }

    #[test]
    /// tests if a packet is not accepted if it and a accepted sent packets exceed max_capacity
    fn test_capacity_exceeding_two_packets() {
        let (sender, _receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data = vec![0; 500];
        sender.try_send(data).unwrap();
        let data = vec![0; 501];
        let err = sender.try_send(data).unwrap_err();
        assert_eq!(err, TrySendError::ChannelFullError);
    }

    #[test]
    /// tests if capacity is equal to max_capacity, no new packets are accepted
    fn test_capacity_full() {
        let (sender, _receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data = vec![0; 1000];
        sender.try_send(data).unwrap();
        let data = vec![0; 501];
        let err = sender.try_send(data).unwrap_err();
        assert_eq!(err, TrySendError::ChannelFullError);
    }

    #[test]
    /// tests if removing a packet works
    fn test_remove() {
        let (sender, receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data_send = vec![50, 20, 70, 20, 22];
        sender.try_send(data_send.clone()).unwrap();
        let data_received = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(data_send, data_received);
    }

    #[test]
    /// tests if remove does timeout if there was no packet sent before
    fn test_remove_no_packet() {
        let (_sender, receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let err = receiver.recv_timeout(Duration::from_millis(50)).unwrap_err();
        assert_eq!(err, RecvTimeoutError::Timeout);
    }

    #[test]
    /// tests if when multiple packets were sent, it is possible to receive all of them
    fn test_remove_two_packets() {
        let (sender, receiver) = byte_bounded_channel(Information::new::<kilobyte>(1));
        let data_send_1 = vec![50, 20, 70, 20, 22];
        let data_send_2 = vec![30, 20, 30, 20, 22];
        sender.try_send(data_send_1.clone()).unwrap();
        sender.try_send(data_send_2.clone()).unwrap();
        let data_received_1 = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        let data_received_2 = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(data_send_1, data_received_1);
        assert_eq!(data_send_2, data_received_2);
    }

    #[test]
    /// Checks if after channel was full and a packet was dropped, it is still possible to recv the first packet.
    fn test_channel_full_does_not_interfere_with_recv() {
        let (sender, receiver) = byte_bounded_channel(Information::new::<byte>(5));
        let data_send_1 = vec![50, 20, 70, 20, 22];
        sender.try_send(data_send_1.clone()).unwrap();
        let data_send_2 = vec![30, 20, 30, 20, 22];
        let err = sender.try_send(data_send_2.clone()).unwrap_err();
        let data_received_1 = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert_eq!(err, TrySendError::ChannelFullError);
        assert_eq!(data_send_1, data_received_1);
    }

    #[test]
    /// Checks if channel capacity is decreased again after recv.
    fn test_channel_capacity_is_decreased_after_recv() {
        let (sender, receiver) = byte_bounded_channel(Information::new::<byte>(5));
        // fill channel to max_capacity
        let data_send_1 = vec![50, 20, 70, 20, 22];
        sender.try_send(data_send_1.clone()).unwrap();
        // send packet which is not accepted
        let data_send_2 = vec![50, 20, 70, 20, 22];
        let err = sender.try_send(data_send_2.clone()).unwrap_err();
        // receive a packet to make space again
        let data_received_1 = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        // send packet to check if there is space again
        let data_send_2 = vec![50, 20, 70, 20, 22];
        sender.try_send(data_send_2.clone()).unwrap();
        assert_eq!(err, TrySendError::ChannelFullError);
        assert_eq!(data_send_1, data_received_1);
    }

    #[test]
    /// tests if returns empty when channel is empty
    fn test_is_empty_when_empty() {
        let (_sender, receiver) = byte_bounded_channel(Information::new::<byte>(5));
        assert!(receiver.is_empty());
    }

    #[test]
    /// tests if returns not empty when channel is not empty
    fn test_is_empty_not_empty() {
        let (sender, receiver) = byte_bounded_channel(Information::new::<byte>(5));
        // fill channel
        let data_send_1 = vec![50, 20, 70, 20, 22];
        sender.try_send(data_send_1.clone()).unwrap();
        assert!(!receiver.is_empty());
    }
}
