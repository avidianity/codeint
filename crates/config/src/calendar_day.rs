use std::fmt;
use std::str::FromStr;

#[cfg(test)]
mod tests;

const SECONDS_PER_DAY: i64 = 86_400;
const DAYS_PER_400_YEARS: i64 = 146_097;
const DAYS_FROM_YEAR_0_MARCH_1_TO_UNIX_EPOCH: i64 = 719_468;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CalendarDay {
    year: i64,
    month: u8,
    day: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{text:?} is not a real day written as YYYY-MM-DD, such as 2026-01-31")]
pub struct CalendarDayError {
    text: String,
}

impl CalendarDay {
    pub fn containing(unix_seconds: i64) -> CalendarDay {
        let days_since_march_1_of_year_0 =
            unix_seconds.div_euclid(SECONDS_PER_DAY) + DAYS_FROM_YEAR_0_MARCH_1_TO_UNIX_EPOCH;
        let era = days_since_march_1_of_year_0.div_euclid(DAYS_PER_400_YEARS);
        let day_of_era = days_since_march_1_of_year_0.rem_euclid(DAYS_PER_400_YEARS);
        let year_of_era =
            (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
        let day_of_march_year =
            day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
        let month_from_march = (5 * day_of_march_year + 2) / 153;
        let day = day_of_march_year - (153 * month_from_march + 2) / 5 + 1;
        let month = if month_from_march < 10 {
            month_from_march + 3
        } else {
            month_from_march - 9
        };
        let year = year_of_era + era * 400 + i64::from(month <= 2);
        CalendarDay {
            year,
            month: small(month),
            day: small(day),
        }
    }

    pub fn start_unix_seconds(self) -> i64 {
        let month = i64::from(self.month);
        let year = self.year - i64::from(month <= 2);
        let era = year.div_euclid(400);
        let year_of_era = year.rem_euclid(400);
        let month_from_march = if month > 2 { month - 3 } else { month + 9 };
        let day_of_march_year = (153 * month_from_march + 2) / 5 + i64::from(self.day) - 1;
        let day_of_era =
            year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_march_year;
        (era * DAYS_PER_400_YEARS + day_of_era - DAYS_FROM_YEAR_0_MARCH_1_TO_UNIX_EPOCH)
            * SECONDS_PER_DAY
    }
}

fn small(value: i64) -> u8 {
    u8::try_from(value).unwrap_or(u8::MAX)
}

fn days_in_month(year: i64, month: u8) -> u8 {
    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    match month {
        2 if leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

impl FromStr for CalendarDay {
    type Err = CalendarDayError;

    fn from_str(text: &str) -> Result<CalendarDay, CalendarDayError> {
        let invalid = || CalendarDayError {
            text: text.to_owned(),
        };
        let number = |digits: &str| {
            digits
                .bytes()
                .all(|byte| byte.is_ascii_digit())
                .then(|| digits.parse::<u16>().ok())
                .flatten()
        };
        let mut parts = text.split('-');
        let (Some(year), Some(month), Some(day), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(invalid());
        };
        if (year.len(), month.len(), day.len()) != (4, 2, 2) {
            return Err(invalid());
        }
        let (Some(year), Some(month), Some(day)) = (number(year), number(month), number(day))
        else {
            return Err(invalid());
        };
        let year = i64::from(year);
        let month = u8::try_from(month).map_err(|_| invalid())?;
        let day = u8::try_from(day).map_err(|_| invalid())?;
        if !(1..=12).contains(&month) || !(1..=days_in_month(year, month)).contains(&day) {
            return Err(invalid());
        }
        Ok(CalendarDay { year, month, day })
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
