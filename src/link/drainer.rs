use core_affinity::{set_for_current, CoreId};
use crossbeam::atomic::AtomicCell;
use log::debug;
use pnet::datalink::DataLinkReceiver;
use std::sync::atomic::{AtomicBool, Ordering};
use thread_priority::{set_current_thread_priority, ThreadPriority};

use crate::{
    byte_bounded_channel::{self, ByteSender},
    cli::StartupMode,
    runtime::Runtime,
};

pub struct Drainer {
    link_id: usize,
    startup_logic: StartupMode,
    next_sender: AtomicCell<Option<ByteSender>>,
    next_ready: AtomicBool,
    max_thread_priority: bool,
}

impl Drainer {
    pub fn new(link_id: usize, startup_logic: StartupMode, initial_sender: ByteSender, max_thread_priority: bool) -> Self {
        let next_sender = AtomicCell::new(Some(initial_sender));
        let next_ready = AtomicBool::new(false);
        Drainer {
            link_id,
            startup_logic,
            next_sender,
            next_ready,
            max_thread_priority,
        }
    }

    pub fn run(&self, mut socket_input: Box<dyn DataLinkReceiver>, core_id: Option<CoreId>) {
        if let Some(core_id) = core_id {
            debug!("Link {}: start drainer (Core: {}).", self.link_id, core_id.id);
            set_for_current(core_id);
        } else {
            debug!("Link {}: start drainer.", self.link_id);
        }
        if self.max_thread_priority {
            set_current_thread_priority(ThreadPriority::Max).expect("Could not set thread priority to MAX.");
        }

        let mut tx = self.next_sender.take().expect("Initially, there is a sender.");

        self.handle_startup(&mut tx, &mut socket_input);

        while let Ok(data) = socket_input.next() {
            if self.next_ready.load(Ordering::Relaxed) {
                tx = self.next_sender.take().unwrap();
                self.next_ready.store(false, Ordering::Relaxed);
            }
            self.handle_packet(&mut tx, data.to_vec());
        }
    }

    fn handle_startup(&self, tx: &mut ByteSender, socket_input: &mut Box<dyn DataLinkReceiver>) {
        if self.startup_logic == StartupMode::AppStart {
            Runtime::access_app_start_time();
            return;
        }

        while !Runtime::is_app_start_time_initialized() {
            if let Ok(data) = socket_input.next() {
                if self.startup_logic.check_packet_satisfies_constraint(data) {
                    // set now as start time
                    Runtime::access_app_start_time();
                }
                self.handle_packet(tx, data.to_vec())
            } else {
                panic!("Could not read from socket");
            };
        }
    }

    fn handle_packet(&self, tx: &mut ByteSender, data: Vec<u8>) {
        if let Err(e) = tx.try_send(data) {
            match e {
                byte_bounded_channel::TrySendError::ChannelFullError => {
                    debug!("Drop packet");
                }
                byte_bounded_channel::TrySendError::ChannelDisconnected => panic!("Channel is disconnected."),
            }
        }
    }
}
