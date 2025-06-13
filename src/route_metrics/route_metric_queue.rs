use std::{collections::VecDeque, fs::File, path::Path, time::Duration};

use eyre::{bail, Context, Result};
use log::debug;
use uom::si::{information::byte, information_rate::byte_per_second, u64::Information};

use crate::route_metrics::route_metric::RouteMetricRaw;

use super::route_metric::RouteMetric;

#[derive(Debug, Clone)]
pub struct RouteMetricQueue {
    last_time: Option<Duration>,
    route_metrics: VecDeque<RouteMetric>,
}

impl RouteMetricQueue {
    pub fn try_load(path: &Path) -> Result<Self> {
        debug!("Start loading data points from {:?}", path);
        let file = File::open(path).with_context(|| format!("Failed to read from input path `{:?}`.", path))?;
        let mut rdr = csv::Reader::from_reader(file);

        let mut route_metrics = RouteMetricQueue {
            last_time: None,
            route_metrics: VecDeque::new(),
        };

        for result in rdr.deserialize() {
            let record: RouteMetricRaw = result?;
            let route_metric: RouteMetric = record.into();
            debug!("Found route_metric: {:?}", route_metric);
            route_metrics.add_route_metric(route_metric)?;
        }

        Ok(route_metrics)
    }

    fn add_route_metric(&mut self, route_metric: RouteMetric) -> Result<()> {
        self.last_time = match self.last_time {
            Some(last_time) if last_time >= route_metric.time_after_start => bail!("Times must be in ascending order and unique!"),
            _ => Some(route_metric.time_after_start),
        };
        self.route_metrics.push_back(route_metric);
        Ok(())
    }

    pub fn peek_next_route_metric(&self) -> Option<&RouteMetric> {
        self.route_metrics.front()
    }

    pub fn pop_next_route_metric(&mut self) -> Option<RouteMetric> {
        self.route_metrics.pop_front()
    }

    pub fn get_max_bdp(&self) -> Information {
        self.route_metrics
            .iter()
            .map(|rm| {
                let bdp = rm.btldr.get::<byte_per_second>() * 2.0 * rm.delay.as_secs_f64();
                Information::new::<byte>(bdp as u64)
            })
            .max()
            .expect("route metrics shouldn't be empty")
    }
}
