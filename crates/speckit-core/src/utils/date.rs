use chrono::{DateTime, Local, TimeZone, Utc};

/// Format a date as a local date string (YYYY-MM-DD).
pub fn format_local_date(date: &DateTime<Utc>) -> String {
    date.with_timezone(&Local).format("%Y-%m-%d").to_string()
}

/// Format a date as a relative time string (e.g., "2 hours ago").
/// Accepts any chrono TimeZone.
pub fn format_relative_time<Tz: TimeZone>(date: &DateTime<Tz>) -> String
where
    Tz::Offset: std::fmt::Display,
{
    let now = Utc::now();
    let duration = now.signed_duration_since(date.with_timezone(&Utc));

    if duration.num_seconds() < 60 {
        "just now".to_string()
    } else if duration.num_minutes() < 60 {
        let mins = duration.num_minutes();
        if mins == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", mins)
        }
    } else if duration.num_hours() < 24 {
        let hours = duration.num_hours();
        if hours == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", hours)
        }
    } else if duration.num_days() < 30 {
        let days = duration.num_days();
        if days == 1 {
            "1 day ago".to_string()
        } else {
            format!("{} days ago", days)
        }
    } else if duration.num_days() < 365 {
        let months = duration.num_days() / 30;
        if months == 1 {
            "1 month ago".to_string()
        } else {
            format!("{} months ago", months)
        }
    } else {
        let years = duration.num_days() / 365;
        if years == 1 {
            "1 year ago".to_string()
        } else {
            format!("{} years ago", years)
        }
    }
}

/// Format a DateTime<Local> as a relative time string.
pub fn format_relative_time_local(date: &DateTime<Local>) -> String {
    format_relative_time(date)
}

/// Get today's date as YYYY-MM-DD.
pub fn today_string() -> String {
    Local::now().format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_local_date() {
        let date = Utc.with_ymd_and_hms(2025, 1, 15, 10, 30, 0).unwrap();
        assert_eq!(format_local_date(&date), "2025-01-15");
    }

    #[test]
    fn test_format_relative_time_just_now() {
        let now = Utc::now();
        assert_eq!(format_relative_time(&now), "just now");
    }

    #[test]
    fn test_today_string() {
        let today = today_string();
        assert_eq!(today.len(), 10);
        assert!(today.contains('-'));
    }
}
