//! Utility functions for time and duration formatting.

/// Convert seconds to human-readable duration
///
/// Handles positive and negative durations, with correct singular/plural forms.
///
/// # Examples
/// - `humanize_duration(1)` → "1 second"
/// - `humanize_duration(90)` → "1 minute"
/// - `humanize_duration(-3600)` → "-1 hour"
pub fn humanize_duration(seconds: i64) -> String {
    let is_negative = seconds < 0;
    let abs_seconds = seconds.abs();
    let prefix = if is_negative { "-" } else { "" };

    let (value, unit) = if abs_seconds < 60 {
        (
            abs_seconds,
            if abs_seconds == 1 {
                "second"
            } else {
                "seconds"
            },
        )
    } else if abs_seconds < 3600 {
        let mins = abs_seconds / 60;
        (mins, if mins == 1 { "minute" } else { "minutes" })
    } else if abs_seconds < 86400 {
        let hours = abs_seconds / 3600;
        (hours, if hours == 1 { "hour" } else { "hours" })
    } else {
        let days = abs_seconds / 86400;
        (days, if days == 1 { "day" } else { "days" })
    };

    format!("{}{} {}", prefix, value, unit)
}
