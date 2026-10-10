//! Linux process peak RSS; absent or malformed measurements remain unavailable.
pub(crate) fn peak_rss(status: &str) -> Option<u64> {
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            return rest
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()?
                .checked_mul(1024);
        }
    }
    None
}
