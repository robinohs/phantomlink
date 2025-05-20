use eyre::{bail, eyre, OptionExt, Result};
use log::info;
use pnet::datalink::{self, Channel, ChannelType, Config, NetworkInterface};
use std::{
    path::Path,
    sync::OnceLock,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use uom::si::information::byte;

use crate::{
    cli::StartupMode,
    link::oneway_virtual_link::{OVLCoreConfig, OnewayVirtualLink},
    route_metrics::RouteMetricQueue,
    ReconfigurationMode,
};

static START_TIME: OnceLock<Instant> = OnceLock::new();

pub struct Runtime {
    route_metric_queue: RouteMetricQueue,
    startup_mode: StartupMode,
    buffer_size_multiplier: f64,
    reconfiguration_delay: Duration,
    reconfiguration_mode: ReconfigurationMode,
}

impl Runtime {
    pub fn access_app_start_time() -> Instant {
        *START_TIME.get_or_init(|| {
            let instant_for_start = Instant::now();
            // write a timestamp to console
            info!(
                "PHANTOMLINK_TS_START={}",
                SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
            );
            instant_for_start
        })
    }

    pub fn is_app_start_time_initialized() -> bool {
        START_TIME.get().is_some()
    }

    /// Creates a new runtime for packet delaying.
    pub fn new(
        input_file_path: &Path,
        startup_mode: StartupMode,
        buffer_size_multiplier: f64,
        reconfiguration_delay: Duration,
        reconfiguration_mode: ReconfigurationMode,
    ) -> Result<Runtime> {
        info!("Creating new phantomlink instance, reading input from {:?}.", input_file_path);

        let rt = Runtime {
            route_metric_queue: RouteMetricQueue::try_load(input_file_path)?,
            startup_mode,
            buffer_size_multiplier,
            reconfiguration_delay,
            reconfiguration_mode,
        };
        Ok(rt)
    }

    /// Starts the runtime loop for packet delaying including listening on the given NICs.
    pub fn run(&mut self) -> Result<()> {
        info!("Starting runtime");

        self.set_socket_parameters()?;

        let client_interf_name = "sim-veth1";
        let server_interf_name = "sim-veth2";

        // channel config
        let config: datalink::Config = Config {
            write_buffer_size: 4096,
            read_buffer_size: 4096,
            read_timeout: None,
            write_timeout: None,
            channel_type: ChannelType::Layer2,
            bpf_fd_attempts: 1000,
            linux_fanout: None,
            promiscuous: true,
            socket_fd: None,
        };

        // Find the network interface with the provided name
        let interface_names_match = |iface: &NetworkInterface| iface.name == client_interf_name;
        let interfaces = datalink::interfaces();
        let interface = interfaces
            .into_iter()
            .find(interface_names_match)
            .ok_or_eyre("Could not find client interface.")?;
        // Create a new channel, dealing with layer 2 packets
        let (tx_client, rx_client) = match datalink::channel(&interface, config) {
            Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => bail!("Unhandled channel type"),
            Err(e) => bail!("An error occurred when creating the datalink channel: {}", e),
        };

        // Find the network interface with the provided name
        let interface_names_match = |iface: &NetworkInterface| iface.name == server_interf_name;
        let interfaces = datalink::interfaces();
        let interface = interfaces
            .into_iter()
            .find(interface_names_match)
            .ok_or_eyre("Could not find server interface.")?;
        // Create a new channel, dealing with layer 2 packets
        let (tx_server, rx_server) = match datalink::channel(&interface, config) {
            Ok(Channel::Ethernet(tx, rx)) => (tx, rx),
            Ok(_) => bail!("Unhandled channel type"),
            Err(e) => bail!("An error occurred when creating the datalink channel: {}", e),
        };

        // Get list of available "cores"
        let mut cores = core_affinity::get_core_ids().ok_or_eyre("Could not get list of available cores")?;
        cores.retain(|core_id| core_id.id >= 2 && core_id.id != 16 && core_id.id != 17); // remove cores 0 & 1 (and their thread siblings) reserved for iperf
        let (cores_1, cores_2) = if cores.len() < 8 {
            info!("Disable CPU core pinning (found {} cores, which is less than 8)", cores.len());
            (None, None)
        } else {
            info!("Enable CPU core pinning (found {} cores)", cores.len());
            // give 50% of cores to each OnewayVirtualLink taking into account thread sibling pairings (e.g. 0 & 16, 1 & 17, ...)
            let (cores_1, cores_2) = cores.into_iter().partition(|core_id| core_id.id % 2 == 0);
            let (cores_1, cores_2) = (OVLCoreConfig::new_from_vec(cores_1), OVLCoreConfig::new_from_vec(cores_2));
            (Some(cores_1), Some(cores_2))
        };

        let mut ovl1 = OnewayVirtualLink::new(0, self.startup_mode, self.buffer_size_multiplier, cores_1);
        let mut ovl2 = OnewayVirtualLink::new(1, self.startup_mode, self.buffer_size_multiplier, cores_2);
        let rmq1 = self.route_metric_queue.clone();
        let rmq2 = self.route_metric_queue.clone();
        let rec_delay = self.reconfiguration_delay;
        let rec_mode = self.reconfiguration_mode;
        let lh1 = thread::spawn(move || {
            ovl1.run(rx_client, tx_server, rmq1, rec_delay, rec_mode).unwrap();
        });
        let lh2 = thread::spawn(move || {
            ovl2.run(rx_server, tx_client, rmq2, rec_delay, rec_mode).unwrap();
        });

        lh1.join().unwrap();
        lh2.join().unwrap();

        info!("Exiting...");
        Ok(())
    }

    fn set_socket_parameters(&self) -> Result<()> {
        let bdp = self.route_metric_queue.get_max_bdp();
        info!("Found max. BDP of {} Bytes. Setting kernel parameters.", bdp.get::<byte>());
        for param in ["rmem_default", "rmem_max", "wmem_default", "wmem_max"] {
            set_kernel_param(&format!("net.core.{param}={}", 3 * bdp.get::<byte>()))?;
        }
        set_kernel_param(&format!("net.ipv4.tcp_rmem=4096 131072 {}", 3 * bdp.get::<byte>()))?;
        set_kernel_param(&format!("net.ipv4.tcp_wmem=4096 16384 {}", 3 * bdp.get::<byte>()))?;
        Ok(())
    }
}

#[track_caller]
fn set_kernel_param(param: &str) -> Result<()> {
    let output = std::process::Command::new("sudo")
        .arg("ip")
        .arg("netns")
        .arg("exec")
        .arg("default")
        .arg("sysctl")
        .arg("-w")
        .arg(param)
        .output()?;
    if output.status.success() {
        info!("{}", std::str::from_utf8(&output.stdout)?.trim_end());
        Ok(())
    } else {
        Err(eyre!(
            "Could not set kernel parameter \"{param}\". Stderr: {}",
            std::str::from_utf8(&output.stderr)?
        ))
    }
}
