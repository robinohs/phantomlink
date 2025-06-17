use crossbeam::channel::Receiver;
use eyre::{bail, OptionExt, Result};
use log::{debug, error, info};
use pnet::datalink::{self, Channel, ChannelType, Config, NetworkInterface};
use std::{
    collections::HashMap,
    io::Write,
    path::Path,
    sync::OnceLock,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tempfile::NamedTempFile;
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
    default_kernel_params: HashMap<String, Vec<String>>,
}

impl Runtime {
    pub fn access_app_start_time() -> Instant {
        *START_TIME.get_or_init(|| {
            let instant_for_start = Instant::now();
            // write a timestamp to console
            info!(
                "Received first valid packet. PHANTOMLINK_TS_START={}",
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
            default_kernel_params: HashMap::new(),
        };
        Ok(rt)
    }

    /// Starts the runtime loop for packet delaying including listening on the given NICs.
    pub fn run(&mut self, stop_rx: Receiver<()>) -> Result<()> {
        debug!("Starting runtime.");

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
            debug!("Disable CPU core pinning (found {} cores, which is less than 8)", cores.len());
            (None, None)
        } else {
            debug!("Enable CPU core pinning (found {} cores)", cores.len());
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
        let _lh1 = thread::spawn(move || {
            ovl1.run(rx_client, tx_server, rmq1, rec_delay, rec_mode).unwrap();
        });
        let _lh2 = thread::spawn(move || {
            ovl2.run(rx_server, tx_client, rmq2, rec_delay, rec_mode).unwrap();
        });

        info!("Phantomlink ready, waiting for first packet...");

        // Handle shutdown signal
        // This thread will block until a shutdown signal is received.
        thread::spawn(move || match stop_rx.recv() {
            Ok(_) => info!("Received shutdown signal, stopping phantomlink..."),
            Err(e) => {
                error!("Error receiving shutdown signal: {}", e);
            }
        })
        .join()
        .unwrap();

        // Restore kernel parameters
        self.restore_kernel_params()?;
        debug!("Kernel parameters restored.");

        info!("Exit...");
        Ok(())
    }

    /// Stores the current value of a kernel parameter and returns it.
    fn store_kernel_param(&mut self, param: &str) -> Result<Vec<String>> {
        let value = get_kernel_param(param)?;
        debug!("Store `{:?}` for `{param}`", value);
        self.default_kernel_params.insert(param.to_string(), value.clone());
        Ok(value)
    }

    /// Restores the kernel parameters to their read values at the start of the runtime.
    fn restore_kernel_params(&self) -> Result<()> {
        info!("Restoring kernel parameters.");
        for (param, value) in &self.default_kernel_params {
            debug!("Restoring `{:?}` for `{param}`", value);
            let values = value.join(" ");
            set_kernel_param(&format!("{param}={values}"))?;
        }
        Ok(())
    }

    /// Updates the kernel parameters based on the maximum BDP found in the route metric queue.
    fn set_socket_parameters(&mut self) -> Result<()> {
        let bdp = self.route_metric_queue.get_max_bdp();
        info!("Found max. BDP of {} Bytes. Setting kernel parameters.", bdp.get::<byte>());
        for param in [
            "net.core.rmem_default",
            "net.core.rmem_max",
            "net.core.wmem_default",
            "net.core.wmem_max",
        ] {
            self.store_kernel_param(param)?;
            set_kernel_param(&format!("{param}={}", 3 * bdp.get::<byte>()))?;
        }
        // tcp_rmem is set to 4096 131072 <max_bdp>
        let values = self.store_kernel_param("net.ipv4.tcp_rmem")?;
        set_kernel_param(&format!(
            "net.ipv4.tcp_rmem={} {} {}",
            values.first().unwrap(),
            values.get(1).unwrap(),
            3 * bdp.get::<byte>()
        ))?;
        // tcp_wmem is set to 4096 16384 <max_bdp>
        let values = self.store_kernel_param("net.ipv4.tcp_wmem")?;
        set_kernel_param(&format!(
            "net.ipv4.tcp_wmem={} {} {}",
            values.first().unwrap(),
            values.get(1).unwrap(),
            3 * bdp.get::<byte>()
        ))?;
        self.write_kernel_params_to_tmp()?;
        Ok(())
    }

    /// Writes the current kernel parameters to a temporary file.
    /// This is used so that users could restore the parameters after the runtime has finished ungracefully.
    fn write_kernel_params_to_tmp(&self) -> Result<()> {
        let tmpfile: NamedTempFile =
            NamedTempFile::new().map_err(|e| eyre::eyre!("Could not create temporary file for kernel parameters: {}", e))?;
        let (mut file, path) = tmpfile
            .keep()
            .map_err(|e| eyre::eyre!("Could not keep temporary file for kernel parameters: {}", e))?;
        for (param, value) in &self.default_kernel_params {
            let value = value.join(" ");
            let line = format!("{param}={value}");
            writeln!(file, "{line}").map_err(|e| eyre::eyre!("Could not write kernel parameter to temporary file: {}", e))?;
        }
        info!("Wrote changed kernel parameters to temporary file: `{:?}` (Owned by root)", path);
        Ok(())
    }
}

#[track_caller]
fn get_kernel_param(param: &str) -> Result<Vec<String>> {
    let output = std::process::Command::new("sudo")
        .arg("ip")
        .arg("netns")
        .arg("exec")
        .arg("default")
        .arg("sysctl")
        .arg(param)
        .output()?;
    if output.status.success() {
        let output_str = std::str::from_utf8(&output.stdout)?
            .trim()
            .split_once("=")
            .ok_or(eyre::eyre!("Could not parse output of kernel parameter \"{param}\"."))?
            .1
            .split_whitespace()
            .map(|s| s.trim().to_string())
            .collect::<Vec<String>>();
        Ok(output_str)
    } else {
        Err(eyre::eyre!(
            "Could not get kernel parameter \"{param}\". Stderr: {}",
            std::str::from_utf8(&output.stderr)?
        ))
    }
}

#[track_caller]
fn set_kernel_param(param: &str) -> Result<()> {
    debug!("Setting kernel parameter: {param}");
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
        debug!("{}", std::str::from_utf8(&output.stdout)?.trim_end());
        Ok(())
    } else {
        Err(eyre::eyre!(
            "Could not set kernel parameter \"{param}\". Stderr: {}",
            std::str::from_utf8(&output.stderr)?
        ))
    }
}
