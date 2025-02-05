use std::{
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
};

use core_affinity::CoreId;
use eyre::OptionExt;
use pnet::datalink::{DataLinkReceiver, DataLinkSender};

use crate::{cli::StartupMode, inflight_queue::InflightQueue, route_metrics::RouteMetricQueue};

use super::{deliverer::Deliverer, dispatcher::Dispatcher};

pub struct OnewayVirtualLink {
    thread_listener: JoinHandle<()>,
    thread_sender: JoinHandle<()>,
}

impl OnewayVirtualLink {
    pub fn new(
        link_id: usize,
        startup_mode: StartupMode,
        buffer_size_multiplier: f64,
        input: Box<dyn DataLinkReceiver>,
        output: Box<dyn DataLinkSender>,
        route_metric_queue: RouteMetricQueue,
        mut core_pool: Option<Vec<CoreId>>,
    ) -> Self {
        // create packet stacks for this link
        let initial_delay = route_metric_queue
            .peek_next_route_metric()
            .ok_or_eyre("No initial RDP input found")
            .unwrap()
            .delay;
        let inflight_queue = Arc::new((Mutex::new(InflightQueue::new(link_id, initial_delay)), Condvar::new()));

        let core_sender = core_pool.as_mut().and_then(|cp| cp.pop());

        // start dispatcher
        let inflight_queue_listener = inflight_queue.clone();
        let mut dispatcher = Dispatcher::new(link_id, startup_mode, buffer_size_multiplier, core_pool);
        let thread_listener = thread::spawn(move || {
            dispatcher.run(input, route_metric_queue, inflight_queue_listener).unwrap();
        });

        // start sender
        let inflight_queue_sender = inflight_queue.clone();
        let mut deliverer = Deliverer::new(link_id, inflight_queue_sender);
        let thread_sender = thread::spawn(move || {
            deliverer.run(output, core_sender);
        });

        Self {
            thread_listener,
            thread_sender,
        }
    }

    pub fn join(self) {
        self.thread_listener.join().unwrap();
        self.thread_sender.join().unwrap();
    }
}
