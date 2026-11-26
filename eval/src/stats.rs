use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Distribution {
    pub count: u64,
    pub mean: f64,
    pub median: f64,
    pub standard_deviation: f64,
    pub p10: f64,
    pub p90: f64,
    pub lowest: f64,
    pub highest: f64,
}

pub(crate) fn percentile(sorted: &[f64], fraction: f64) -> f64 {
    let position = fraction * (sorted.len() - 1) as f64;
    let lower = position.floor() as usize;
    let upper = position.ceil() as usize;
    sorted[lower] + (sorted[upper] - sorted[lower]) * (position - lower as f64)
}

impl Distribution {
    pub fn of(values: &[f64]) -> Option<Distribution> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by(f64::total_cmp);
        let count = sorted.len() as f64;
        let mean = sorted.iter().sum::<f64>() / count;
        let variance = sorted
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / count;
        Some(Distribution {
            count: sorted.len() as u64,
            mean,
            median: percentile(&sorted, 0.5),
            standard_deviation: variance.sqrt(),
            p10: percentile(&sorted, 0.10),
            p90: percentile(&sorted, 0.90),
            lowest: sorted[0],
            highest: sorted[sorted.len() - 1],
        })
    }
}

#[cfg(test)]
mod tests;
