use std::time::Duration;

use serde::Deserialize;
use uom::si::{information_rate::byte_per_second, u64::InformationRate};

#[derive(Debug, Deserialize)]
pub struct RouteMetricRaw {
    #[serde(rename = "Time[ms]")]
    time: u64,
    #[serde(rename = "RouteID")]
    route_id: u64,
    #[serde(rename = "Delay[ms]")]
    delay: f64,
    #[serde(rename = "Btldr[Mbps]")]
    btldr: f64,
}

#[derive(Debug, Copy, Clone)]
pub struct RouteMetric {
    pub time_after_start: Duration,
    pub route_id: u64,
    pub delay: Duration,
    pub btldr: InformationRate,
}

impl From<RouteMetricRaw> for RouteMetric {
    fn from(value: RouteMetricRaw) -> Self {
        let delay_ns = (value.delay * 1000.0 * 1000.0) as u64; // first convert ms to ns, then round to u64
        let btldr_byte_s = (value.btldr * (1000.0 * 1000.0 / 8.0)) as u64; // first convert Mbit/s to B/s, then round to u64
        Self {
            time_after_start: Duration::from_millis(value.time),
            route_id: value.route_id,
            delay: Duration::from_nanos(delay_ns),
            btldr: InformationRate::new::<byte_per_second>(btldr_byte_s),
        }
    }
}

#[cfg(test)]
mod tests {
    use uom::si::information_rate::megabit_per_second;

    use super::*;

    #[test]
    fn from_raw() {
        let route_metric_raw = RouteMetricRaw {
            time: 0,
            route_id: 0,
            delay: 140.0,
            btldr: 100.0,
        };

        let route_metric: RouteMetric = route_metric_raw.into();
        assert_eq!(route_metric.time_after_start, Duration::from_millis(0));
        assert_eq!(route_metric.route_id, 0);
        assert_eq!(route_metric.delay, Duration::from_millis(140));
        assert_eq!(route_metric.btldr, InformationRate::new::<megabit_per_second>(100));
    }
}
