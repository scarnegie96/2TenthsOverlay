//! Dates and times as the web's `toLocaleString` shows them, in the local
//! time zone: "Nov 1, 2026, 3:04 PM", "1 Nov 2026".

use chrono::{DateTime, Local, TimeZone};

fn local(secs: u64) -> DateTime<Local> {
    Local.timestamp_opt(secs as i64, 0).single().unwrap_or_else(|| Local.timestamp_opt(0, 0).unwrap())
}

fn fmt_medium<Tz: TimeZone>(t: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    // `%-d` and `%-I` (no padding) work in chrono on every platform.
    t.format("%b %-d, %Y, %-I:%M %p").to_string()
}

fn fmt_full<Tz: TimeZone>(t: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    t.format("%A, %B %-d, %Y at %-I:%M %p").to_string()
}

fn fmt_date<Tz: TimeZone>(t: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    t.format("%-d %b %Y").to_string()
}

/// "Nov 1, 2026, 3:04 PM"
pub fn medium(secs: u64) -> String {
    fmt_medium(&local(secs))
}

/// "Sunday, November 1, 2026 at 3:04 PM"
pub fn full(secs: u64) -> String {
    fmt_full(&local(secs))
}

/// "1 Nov 2026"
pub fn date_medium(secs: u64) -> String {
    fmt_date(&local(secs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn formats() {
        // 2026-10-31 14:25:01 UTC
        let t = Utc.timestamp_opt(1_793_456_701, 0).unwrap();
        assert_eq!(fmt_medium(&t), "Oct 31, 2026, 2:25 PM");
        assert_eq!(fmt_full(&t), "Saturday, October 31, 2026 at 2:25 PM");
        assert_eq!(fmt_date(&t), "31 Oct 2026");
        let t = Utc.timestamp_opt(0, 0).unwrap();
        assert_eq!(fmt_medium(&t), "Jan 1, 1970, 12:00 AM");
        // The local-time versions don't panic.
        assert!(!medium(1_793_456_701).is_empty() && !full(5).is_empty() && !date_medium(0).is_empty());
    }
}
