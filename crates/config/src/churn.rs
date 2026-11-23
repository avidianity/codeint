use std::fmt;
use std::str::FromStr;

use crate::CalendarDay;

pub const DEFAULT_CHURN_DAYS: u32 = 90;

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ChurnFormat {
    Text,
    Toon,
    Json,
    Markdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChurnSince {
    Days(u32),
    Day(CalendarDay),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChurnWindow {
    Days(u32),
    Since(CalendarDay),
    Commits(usize),
}

impl Default for ChurnWindow {
    fn default() -> ChurnWindow {
        ChurnWindow::Days(DEFAULT_CHURN_DAYS)
    }
}

impl From<ChurnSince> for ChurnWindow {
    fn from(since: ChurnSince) -> ChurnWindow {
        match since {
            ChurnSince::Days(days) => ChurnWindow::Days(days),
            ChurnSince::Day(day) => ChurnWindow::Since(day),
        }
    }
}

impl fmt::Display for ChurnWindow {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChurnWindow::Days(days) => write!(formatter, "{days}d"),
            ChurnWindow::Since(day) => write!(formatter, "{day}"),
            ChurnWindow::Commits(1) => write!(formatter, "1 commit"),
            ChurnWindow::Commits(count) => write!(formatter, "{count} commits"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
    "{text:?} is not a positive day count such as 90d or a real UTC date written as YYYY-MM-DD"
)]
pub struct ChurnSinceError {
    text: String,
}

impl FromStr for ChurnSince {
    type Err = ChurnSinceError;

    fn from_str(text: &str) -> Result<ChurnSince, ChurnSinceError> {
        if let Some(prefix) = text.strip_suffix('d').or_else(|| text.strip_suffix('D')) {
            if let Ok(days) = prefix.parse::<u32>()
                && days > 0
            {
                return Ok(ChurnSince::Days(days));
            }
            return Err(ChurnSinceError {
                text: text.to_owned(),
            });
        }
        CalendarDay::from_str(text)
            .map(ChurnSince::Day)
            .map_err(|_| ChurnSinceError {
                text: text.to_owned(),
            })
    }
}
