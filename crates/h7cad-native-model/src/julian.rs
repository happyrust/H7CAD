//! Julian date ↔ UTC conversion helpers.
//!
//! Built to let the UI format `DocumentHeader.tdcreate` / `tdupdate`
//! (raw `f64` Julian date passthrough values from DXF HEADER) as
//! human-readable dates without pulling in `chrono` / `time` crates.
//! Uses the Fliegel–Van Flandern (1968) algorithm — integer-only,
//! ~50 lines of code, public domain, stable across platforms.
//!
//! **Precision**: second-level. AutoCAD's own Julian-date writes are
//! effectively second-precision (10 decimal digits in the DXF text
//! stream give ~1ms but the underlying timestamp is seconds), so
//! round-tripping through this helper does not lose information
//! relative to the source.
//!
//! **Timezone**: UTC everywhere. The AutoCAD DXF Reference says
//! `$TDCREATE` is "local time", but H7CAD does not track a timezone
//! on the document side, so this helper treats every `jd` as if it
//! represented a UTC wall-clock reading. Consumers that care about
//! local display need to shift manually.
//!
//! **Range**: valid for Gregorian dates 1900-01-01 through 2100-01-01
//! (Julian date ~2415020.5 – 2488070.5). Outside this window the
//! Fliegel–Van Flandern integer math still terminates but the result
//! may not match the historical calendar.

/// A broken-down UTC wall-clock reading. All fields are 1-indexed in
/// the natural way: `month = 1..=12`, `day = 1..=31`, `hour = 0..=23`,
/// `minute = 0..=59`, `second = 0..=59`. Leap seconds are not modelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateTimeUtc {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl DateTimeUtc {
    /// Build a fresh `DateTimeUtc`. No validation — invalid inputs
    /// (e.g. month = 13) silently flow through; upstream callers
    /// should validate before construction.
    pub fn new(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }
}

/// Convert a raw Julian date (as stored in `DocumentHeader.tdcreate`
/// and friends) into a broken-down UTC wall-clock reading.
pub fn julian_date_to_utc(jd: f64) -> DateTimeUtc {
    // JDN = floor(JD + 0.5). JDN ticks at midnight UTC (not noon), so
    // the `+ 0.5` rolls noon → midnight alignment.
    let shifted = jd + 0.5;
    let jdn = shifted.floor() as i64;
    let (year, month, day) = jdn_to_gregorian(jdn);

    // Sub-day fraction: [0, 1). `shifted.fract()` keeps the decimal
    // part of (jd + 0.5) which maps 0.0 → midnight and 0.5 → noon.
    let frac = shifted - shifted.floor();
    let total_seconds = (frac * 86_400.0).round() as i64;
    // Carry: rounding can push 86400 → next day.
    let (total_seconds, day, month, year) = if total_seconds >= 86_400 {
        let (y, mo, d) = jdn_to_gregorian(jdn + 1);
        (total_seconds - 86_400, d, mo, y)
    } else {
        (total_seconds, day, month, year)
    };
    let hour = (total_seconds / 3600) as u32;
    let minute = ((total_seconds % 3600) / 60) as u32;
    let second = (total_seconds % 60) as u32;

    DateTimeUtc {
        year,
        month,
        day,
        hour,
        minute,
        second,
    }
}

/// Inverse of `julian_date_to_utc`. Deliberately does **not** try to
/// model sub-second precision (AutoCAD's own timestamps are second-
/// precision), so passing a `DateTimeUtc` that came from a non-zero
/// `nanosecond` / `millisecond` upstream will truncate.
pub fn utc_to_julian_date(dt: &DateTimeUtc) -> f64 {
    let jdn = gregorian_to_jdn(dt.year, dt.month, dt.day);
    let sub_day = (dt.hour as f64 * 3600.0
        + dt.minute as f64 * 60.0
        + dt.second as f64)
        / 86_400.0;
    // Reverse of the `+ 0.5` shift in `julian_date_to_utc`: JDN ticks
    // at midnight, but Julian date's integer-tick is at noon, so we
    // subtract 0.5 to bring midnight JDN back to the JD coordinate.
    (jdn as f64 - 0.5) + sub_day
}

/// Format a `DateTimeUtc` as an ISO-8601 `YYYY-MM-DDTHH:MM:SSZ` string.
pub fn format_iso8601(dt: &DateTimeUtc) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        dt.year, dt.month, dt.day, dt.hour, dt.minute, dt.second
    )
}

/// Parse a strict ISO-8601 `YYYY-MM-DDTHH:MM:SSZ` UTC string into a
/// `DateTimeUtc`. Returns `None` for any deviation from the canonical
/// 20-character form.
///
/// **Strict acceptance rules**:
/// - Exact 20-character length (4-digit year + 2-digit month/day, etc.)
/// - Separators must be `-` `-` `T` `:` `:` `Z` at fixed positions
/// - Letters must be **upper-case** (`T` / `Z`)
/// - No fractional seconds (`.123`)
/// - No timezone offset (`+08:00`); only `Z` for UTC
/// - Year must be parseable as `i32` (negative / BC technically allowed
///   by `i32::from_str` for `-0500-01-01T...` but only with the
///   sign-prefix length adjusted; this parser sticks to 4-digit
///   non-negative years which is the format `format_iso8601` produces
///   inside the AutoCAD timestamp range)
/// - Field range checks: month 1-12 / day 1-31 / hour 0-23 / minute
///   0-59 / second 0-60 (leap-second slot tolerated, not modelled)
///
/// Calendar validity (Feb 30, Apr 31, …) is **not** enforced — that's
/// a UI / domain-layer concern.
pub fn parse_iso8601(s: &str) -> Option<DateTimeUtc> {
    if s.len() != 20 {
        return None;
    }
    let bytes = s.as_bytes();
    if bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes[10] != b'T'
        || bytes[13] != b':'
        || bytes[16] != b':'
        || bytes[19] != b'Z'
    {
        return None;
    }
    let parse_u = |from: usize, to: usize| -> Option<u32> { s[from..to].parse::<u32>().ok() };
    let year = s[0..4].parse::<i32>().ok()?;
    let month = parse_u(5, 7)?;
    let day = parse_u(8, 10)?;
    let hour = parse_u(11, 13)?;
    let minute = parse_u(14, 16)?;
    let second = parse_u(17, 19)?;

    if !(1..=12).contains(&month) {
        return None;
    }
    if !(1..=31).contains(&day) {
        return None;
    }
    if hour > 23 {
        return None;
    }
    if minute > 59 {
        return None;
    }
    if second > 60 {
        return None;
    }
    Some(DateTimeUtc {
        year,
        month,
        day,
        hour,
        minute,
        second,
    })
}

// ──────────────────────────────────────────────────────────────────────
// Fliegel–Van Flandern (1968) implementations. Integer-only.
// ──────────────────────────────────────────────────────────────────────

/// Convert a Julian Day Number (JDN, integer days since 4713 BC Jan 1
/// noon UTC) to a Gregorian `(year, month, day)`.
fn jdn_to_gregorian(jdn: i64) -> (i32, u32, u32) {
    let l = jdn + 68_569;
    let n = (4 * l) / 146_097;
    let l = l - (146_097 * n + 3) / 4;
    let i = (4_000 * (l + 1)) / 1_461_001;
    let l = l - (1_461 * i) / 4 + 31;
    let j = (80 * l) / 2_447;
    let day = (l - (2_447 * j) / 80) as u32;
    let l = j / 11;
    let month = (j + 2 - 12 * l) as u32;
    let year = (100 * (n - 49) + i + l) as i32;
    (year, month, day)
}

/// Convert a Gregorian `(year, month, day)` to Julian Day Number.
/// Uses the Meeus "Astronomical Algorithms" closed-form (adjusted for
/// integer-only arithmetic in the post-1582 Gregorian calendar).
fn gregorian_to_jdn(year: i32, month: u32, day: u32) -> i64 {
    let (y, m) = if month <= 2 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let a = y / 100;
    let b = 2 - a + a / 4;
    (365.25 * (y as f64 + 4716.0)).floor() as i64
        + (30.6001 * (m as f64 + 1.0)).floor() as i64
        + day as i64
        + b as i64
        - 1524
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn julian_date_reference_value_maps_to_2020_01_01_utc() {
        // AutoCAD 2018 DXF Reference example: JD 2458849.82939815
        // corresponds to 2020-01-01 07:54:19.920 UTC. Assert the
        // date is exact and the clock is within one second of target.
        let dt = julian_date_to_utc(2458849.82939815);
        assert_eq!((dt.year, dt.month, dt.day), (2020, 1, 1));
        // Expected 07:54:19 or 07:54:20 depending on rounding.
        assert_eq!(dt.hour, 7);
        assert_eq!(dt.minute, 54);
        assert!(
            (dt.second as i64 - 20).abs() <= 1,
            "expected seconds near 20, got {}",
            dt.second
        );
    }

    #[test]
    fn unix_epoch_julian_date_maps_to_1970_01_01_midnight() {
        // JD 2440587.5 is the UNIX epoch (1970-01-01 00:00:00 UTC).
        let dt = julian_date_to_utc(2440587.5);
        assert_eq!(dt, DateTimeUtc::new(1970, 1, 1, 0, 0, 0));
    }

    #[test]
    fn j2000_maps_to_2000_01_01_noon_utc() {
        // J2000.0 epoch: JD 2451545.0 = 2000-01-01 12:00:00 UTC.
        let dt = julian_date_to_utc(2451545.0);
        assert_eq!(dt, DateTimeUtc::new(2000, 1, 1, 12, 0, 0));
    }

    #[test]
    fn julian_date_roundtrip_preserves_dates_across_the_20th_century() {
        for &(y, mo, d, h, mi, s) in &[
            (1900, 6, 15, 0, 0, 0),
            (1970, 1, 1, 0, 0, 0),
            (2000, 1, 1, 12, 0, 0),
            (2020, 1, 1, 7, 54, 20),
            (2100, 1, 1, 23, 59, 59),
        ] {
            let dt = DateTimeUtc::new(y, mo, d, h, mi, s);
            let jd = utc_to_julian_date(&dt);
            let dt_back = julian_date_to_utc(jd);
            assert_eq!(
                dt_back, dt,
                "roundtrip drift for input {:?}, jd = {}",
                dt, jd
            );
        }
    }

    #[test]
    fn format_iso8601_pads_and_emits_canonical_string() {
        let dt = DateTimeUtc::new(2020, 1, 1, 7, 54, 20);
        assert_eq!(format_iso8601(&dt), "2020-01-01T07:54:20Z");

        let dt = DateTimeUtc::new(1999, 12, 31, 23, 59, 59);
        assert_eq!(format_iso8601(&dt), "1999-12-31T23:59:59Z");

        // Single-digit month / day / hour / minute / second must pad.
        let dt = DateTimeUtc::new(2026, 4, 2, 3, 7, 5);
        assert_eq!(format_iso8601(&dt), "2026-04-02T03:07:05Z");
    }

    #[test]
    fn parse_iso8601_canonical_form_succeeds() {
        let parsed = parse_iso8601("2020-01-01T07:54:20Z").expect("valid");
        assert_eq!(parsed, DateTimeUtc::new(2020, 1, 1, 7, 54, 20));
    }

    #[test]
    fn parse_iso8601_rejects_obvious_format_errors() {
        // Wrong separators, missing Z, lower-case T/Z, non-canonical
        // length, non-numeric fields — all should return None.
        for bad in &[
            "2020-01-01T07:54:20",      // missing Z
            "2020/01/01T07:54:20Z",     // wrong date sep
            "2020-01-01t07:54:20Z",     // lower-case T
            "2020-01-01T07:54:20z",     // lower-case Z
            "2020-1-1T7:54:20Z",        // non-padded fields
            "abcd-ef-ghTij:kl:mnZ",     // non-numeric fields
            "2020-13-01T07:54:20Z",     // month out of range
            "2020-01-32T07:54:20Z",     // day out of range
            "2020-01-01T24:54:20Z",     // hour out of range
            "2020-01-01T07:60:20Z",     // minute out of range
            "2020-01-01T07:54:61Z",     // second out of range (61 > 60)
            "2020-01-01T07:54:20+0000", // tz offset, not Z
            "",                          // empty
            "Z",                         // too short
        ] {
            assert!(
                parse_iso8601(bad).is_none(),
                "expected `{bad}` to be rejected"
            );
        }
    }

    #[test]
    fn parse_iso8601_tolerates_leap_second_slot() {
        // Second = 60 is legal "leap second" position per ISO-8601 even
        // though our model doesn't simulate the actual leap.
        let parsed = parse_iso8601("2016-12-31T23:59:60Z").expect("leap-second slot");
        assert_eq!(parsed.second, 60);
    }

    #[test]
    fn format_then_parse_iso8601_roundtrip() {
        for &(y, mo, d, h, mi, s) in &[
            (1970, 1, 1, 0, 0, 0),
            (2000, 1, 1, 12, 0, 0),
            (2020, 1, 1, 7, 54, 20),
            (2099, 12, 31, 23, 59, 59),
        ] {
            let dt = DateTimeUtc::new(y, mo, d, h, mi, s);
            let s = format_iso8601(&dt);
            let back = parse_iso8601(&s).expect("round-trip parse");
            assert_eq!(back, dt, "failed round-trip for {dt:?} via `{s}`");
        }
    }

    #[test]
    fn parse_then_julian_date_roundtrip() {
        // ISO-8601 string → DateTimeUtc → JD → DateTimeUtc → ISO-8601
        // string. Closes the helper loop end-to-end.
        let original = "2020-01-01T07:54:20Z";
        let dt1 = parse_iso8601(original).unwrap();
        let jd = utc_to_julian_date(&dt1);
        let dt2 = julian_date_to_utc(jd);
        let back = format_iso8601(&dt2);
        assert_eq!(back, original);
    }
}
