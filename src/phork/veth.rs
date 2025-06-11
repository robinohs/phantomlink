use crate::phork::{namespace::NS_NAME_LINK, utils::*};
use color_eyre::eyre::Result;
use std::process::Output;

pub(crate) struct VEth<'a> {
    name: &'a str,
    namespace: &'a str,
    ip: &'a str,
    mac: &'a str,
    name_sim: String,
    ip_sim: &'a str,
    mac_sim: &'a str,
}

impl<'a> VEth<'a> {
    pub(crate) fn new(name: &'a str, namespace: &'a str, ip: &'a str, mac: &'a str, ip_sim: &'a str, mac_sim: &'a str) -> Self {
        Self {
            name,
            namespace,
            ip,
            mac,
            name_sim: format!("sim-{}", name),
            ip_sim,
            mac_sim,
        }
    }

    fn sim(&self) -> &str {
        &self.name_sim
    }

    pub(crate) fn create(&self) -> Result<()> {
        link(&[
            "add",
            self.name,
            "netns",
            self.namespace,
            "type",
            "veth",
            "peer",
            "name",
            self.sim(),
        ])?;

        // move sim-veth into phantomlink namespace
        link(&["set", self.sim(), "netns", NS_NAME_LINK])?;

        // set IP
        netns(&["exec", self.namespace, "ip", "addr", "add", self.ip, "dev", self.name])?;

        // set mac
        netns(&[
            "exec",
            self.namespace,
            "ip",
            "link",
            "set",
            "dev",
            self.name,
            "address",
            self.mac,
            "promisc",
            "on",
        ])?;

        // configure sim-veth
        netns(&["exec", NS_NAME_LINK, "ip", "addr", "add", self.ip_sim, "dev", self.sim()])?;
        netns(&[
            "exec",
            NS_NAME_LINK,
            "ip",
            "link",
            "set",
            "dev",
            self.sim(),
            "address",
            self.mac_sim,
        ])?;
        Ok(())
    }

    pub(crate) fn set_default_route(&self) -> Result<Output> {
        netns(&["exec", self.namespace, "route", "add", "default", "dev", self.name])
    }

    pub(crate) fn set_interfaces_up(&self) -> Result<()> {
        netns(&["exec", self.namespace, "ip", "link", "set", self.name, "up"])?;
        netns(&["exec", NS_NAME_LINK, "ip", "link", "set", self.sim(), "up"])?;
        Ok(())
    }

    pub(crate) fn set_loopback_up(&self) -> Result<Output> {
        netns(&["exec", self.namespace, "ip", "link", "set", "lo", "up"])
    }

    pub(crate) fn disable_offload(&self) -> Result<()> {
        let interfaces = [(self.namespace, self.name), (NS_NAME_LINK, self.sim())];
        for (ns, interface) in interfaces {
            netns(&["exec", ns, "ethtool", "-K", interface, "tso", "off"])?;
            netns(&["exec", ns, "ethtool", "--offload", interface, "rx", "off", "tx", "off"])?;
        }
        Ok(())
    }
}
