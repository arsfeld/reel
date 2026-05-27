use std::time::{SystemTime, UNIX_EPOCH};

/// Generate a UTC ISO 8601 timestamp string.
pub fn iso_now() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    // Simple ISO format sufficient for sort ordering
    let secs_per_day = 86400;
    let days_since_epoch = now / secs_per_day;
    let time_of_day = now % secs_per_day;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    // Approximate date calculation (sufficient for timestamping)
    let (year, month, day) = days_to_ymd(days_since_epoch);
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
pub fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Parse a text/uri-list string into individual file:// URIs (or plain paths).
pub fn parse_uri_list(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            if let Some(stripped) = line.strip_prefix("file://") {
                // URL-decode the path (basic: replace %XX)
                urlencoding_decode(stripped)
            } else {
                line.to_string()
            }
        })
        .collect()
}

/// Minimal percent-decode for file:// URLs.
pub fn urlencoding_decode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hi = chars.next().and_then(|c| c.to_digit(16));
            let lo = chars.next().and_then(|c| c.to_digit(16));
            if let (Some(hi), Some(lo)) = (hi, lo) {
                out.push(char::from_u32((hi << 4) | lo).unwrap_or('\u{FFFD}'));
            } else {
                out.push('%');
                if let Some(c) = chars.next() {
                    out.push(c);
                }
            }
        } else {
            out.push(c);
        }
    }
    out
}
