/// Format large iteration counts with appropriate suffixes (K, M, B, T)
pub fn format_iterations(n: u64) -> String {
    if n >= 1_000_000_000_000 {
        format!("{:.2}T", n as f64 / 1_000_000_000_000.0)
    } else if n >= 1_000_000_000 {
        format!("{:.2}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.2}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.2}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Parse an iteration count the way [`format_iterations`] writes them:
/// plain integers, or K/M/B/T suffixes (case-insensitive), with commas,
/// underscores and spaces tolerated. The inverse the suffixed display
/// never had — any widget whose formatter shows "30M" must parse "30M"
/// back, or typing into it round-trips through garbage.
pub fn parse_iterations(s: &str) -> Option<f64> {
    let cleaned: String = s
        .trim()
        .chars()
        .filter(|c| !matches!(c, ',' | '_' | ' '))
        .collect();
    let upper = cleaned.to_uppercase();
    let (num, mult) = if let Some(n) = upper.strip_suffix('T') {
        (n, 1e12)
    } else if let Some(n) = upper.strip_suffix('B') {
        (n, 1e9)
    } else if let Some(n) = upper.strip_suffix('M') {
        (n, 1e6)
    } else if let Some(n) = upper.strip_suffix('K') {
        (n, 1e3)
    } else {
        (upper.as_str(), 1.0)
    };
    let v = num.parse::<f64>().ok()? * mult;
    (v > 0.0).then_some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reported bug: the max-iterations slider's VALUE is
    /// log10(iterations), so VKB-submitted text like "10000000" was
    /// parsed as the LOG and clamped to the slider max — every submit
    /// became 1T. The fix routes typed text through parse_iterations
    /// and takes log10 at the slider's custom_parser.
    #[test]
    fn typed_iteration_counts_round_trip_through_log_space() {
        for (text, expected) in [
            ("10000000", 1e7),
            ("10,000,000", 1e7),
            ("30M", 30e6),
            ("2.5B", 2.5e9),
            ("1T", 1e12),
            ("500k", 5e5),
            (" 1_000_000 ", 1e6),
        ] {
            let v = parse_iterations(text).unwrap_or_else(|| panic!("{text} failed to parse"));
            assert!(
                (v - expected).abs() < expected * 1e-9,
                "{text}: got {v}, expected {expected}"
            );
            // What the slider actually stores.
            let log = v.log10();
            assert!(
                (10f64.powf(log) - expected).abs() < expected * 1e-6,
                "{text} did not survive log-space round trip"
            );
        }
    }

    /// The formatter's own output must parse back (desktop users edit
    /// the displayed "30.00M" text in place).
    #[test]
    fn formatted_output_parses_back() {
        for n in [999u64, 30_000_000, 2_500_000_000, 1_000_000_000_000] {
            let shown = format_iterations(n);
            let parsed = parse_iterations(&shown)
                .unwrap_or_else(|| panic!("formatter output {shown:?} failed to parse"));
            assert!(
                (parsed - n as f64).abs() <= n as f64 * 0.005 + 0.5,
                "{n} -> {shown} -> {parsed}"
            );
        }
    }

    /// Garbage and non-positive values are rejected, not converted.
    #[test]
    fn junk_is_rejected() {
        for bad in ["", "abc", "-5", "0", "1Q", "M"] {
            assert!(parse_iterations(bad).is_none(), "{bad:?} should not parse");
        }
    }
}
