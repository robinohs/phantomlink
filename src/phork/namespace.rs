use eyre;
use libc::{setns, CLONE_NEWNET};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use crate::phork::utils::*;
use crate::phork::veth::VEth;

pub const NS_NAME_LINK: &str = "pl_link";
pub const NS_NAME_CLIENT: &str = "pl_client";
pub const NS_NAME_SERVER: &str = "pl_server";
const NAMESPACES: [&str; 3] = [NS_NAME_CLIENT, NS_NAME_LINK, NS_NAME_SERVER];
const VETH_1: &str = "veth1";
const VETH_2: &str = "veth2";

/// Checks if the network environment is set up.
pub(crate) fn is_setup() -> eyre::Result<bool> {
    for ns in NAMESPACES {
        if Namespace::try_exists(ns)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Sets up the network namespaces and virtual ethernet links for the phork environment.
pub(crate) fn setup() -> eyre::Result<()> {
    // create namespaces
    for ns in NAMESPACES {
        Namespace::try_create(ns)?;
    }

    // configure virtual ethernet links
    let veths = [
        VEth::new(
            VETH_1,
            NS_NAME_CLIENT,
            "192.168.22.2/24",
            "22:22:22:22:22:22",
            "192.168.33.2/24",
            "44:22:22:22:22:22",
        ),
        VEth::new(
            VETH_2,
            NS_NAME_SERVER,
            "192.168.66.2/24",
            "66:66:66:66:66:66",
            "192.168.33.3/24",
            "44:66:66:66:66:66",
        ),
    ];

    // create virtual ethernet link
    for veth in &veths {
        veth.create()?;
    }

    // configure interfaces
    for veth in veths {
        veth.set_interfaces_up()?;
        veth.set_loopback_up()?;
        veth.disable_offload()?;
        veth.set_default_route()?;
    }

    let ns_id = 1;
    // // TODO: get NS id symlink
    exec("ln", &["-sf", &format!("/proc/{}/ns/net", ns_id), "/var/run/netns/default"])?;

    Ok(())
}

/// Cleans up the network namespaces and virtual ethernet links created by the setup function.
pub(crate) fn clean() -> eyre::Result<()> {
    for ns in NAMESPACES {
        match Namespace::try_load(ns) {
            Ok(namespace) => {
                namespace.try_delete()?;
            }
            Err(e) => {
                if e.to_string().contains("does not exist") {
                    // Namespace does not exist, nothing to delete
                    continue;
                } else {
                    return Err(e);
                }
            }
        }
    }

    // Clean up default namespace symlink
    let default_ns_path = Path::new("/var/run/netns/default");
    if default_ns_path.exists() {
        std::fs::remove_file(default_ns_path)?;
    }
    Ok(())
}

/// Represents a network namespace.
pub struct Namespace {
    name: String,
}

impl Namespace {
    /// Creates a new Namespace instance with the given name.
    pub fn try_create(name: &str) -> eyre::Result<Self> {
        if Namespace::try_exists(name)? {
            return Err(eyre::eyre!("Namespace '{}' already exists", name));
        }
        netns(&["add", name])?;
        Ok(Namespace { name: name.to_string() })
    }

    /// Loads an existing Namespace instance by name.
    pub fn try_load(name: &str) -> eyre::Result<Self> {
        if !Namespace::try_exists(name)? {
            return Err(eyre::eyre!("Namespace '{}' does not exist", name));
        }
        Ok(Namespace { name: name.to_string() })
    }

    /// Checks if a namespace with the given name exists.
    pub fn try_exists(name: &str) -> eyre::Result<bool> {
        Ok(Namespace { name: name.to_string() }.path().try_exists()?)
    }

    /// Deletes the namespace.
    pub fn try_delete(self) -> eyre::Result<()> {
        netns(&["del", &self.name])?;
        Ok(())
    }

    /// Moves the calling process to the network namespace.
    pub fn try_switch_calling_pid_to_namespace(&self) -> eyre::Result<()> {
        let ns_path = self.path();
        let ns_fd = File::open(ns_path).map_err(|e| eyre::eyre!("Failed to open namespace file: {}", e))?;
        unsafe {
            if setns(ns_fd.as_raw_fd(), CLONE_NEWNET) == -1 {
                return Err(eyre::eyre!("Failed to set network namespace: {}", std::io::Error::last_os_error()));
            }
        }
        Ok(())
    }

    /// Returns the fs path to the namespace.
    fn path(&self) -> PathBuf {
        Path::new(&format!("/var/run/netns/{}", self.name)).to_path_buf()
    }
}
