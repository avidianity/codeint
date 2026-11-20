use std::fmt;
use std::str::FromStr;

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalendarDay {
    year: u16,
    month: u8,
    day: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{text:?} is not a date in the form YYYY-MM-DD, such as 2026-01-31")]
pub struct CalendarDayError {
    text: String,
}

impl CalendarDay {
    pub fn containing(unix_seconds: i64) -> CalendarDay {
        let _ = unix_seconds;
        todo!()
    }

    pub fn start_unix_seconds(self) -> i64 {
        todo!()
    }
}

impl FromStr for CalendarDay {
    type Err = CalendarDayError;

    fn from_str(text: &str) -> Result<CalendarDay, CalendarDayError> {
        let _ = text;
        todo!()
    }
}

impl fmt::Display for CalendarDay {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:04}-{:02}-{:02}",
            self.year, self.month, self.day
        )
    }
}
