//! Filesystem/process adapter for the explicitly opted-in research collector.
use std::path::Path;

/// Shared formatting at experimental I/O boundaries.
pub(super) fn error_text(error: impl std::fmt::Display) -> String {
    error.to_string()
}
/// Collect Rust libtest coverage in a fresh artifact directory. Python owns
/// bounded, timed process groups; no specification or task evidence is published.
pub fn collect_experimental_coverage(
    root: &Path,
    package: Option<&str>,
    filter: Option<&str>,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    if timeout_secs == 0 {
        return Err("Timeout must be positive".into());
    }
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(error_text)?
        .as_nanos();
    let artifacts = root.join(format!(".dctx0ctl/coverage/{nonce}-{}", std::process::id()));
    let options = serde_json::json!({"root":root,"artifacts":artifacts,"package":package,"filter":filter,"timeout_secs":timeout_secs,"stream":true});
    let result = std::process::Command::new("python3")
        .arg("-c")
        .arg(include_str!("coverage_collect.py"))
        .arg(options.to_string())
        .output()
        .map_err(error_text)?;
    if !result.status.success() {
        return Err(String::from_utf8_lossy(&result.stderr).into_owned());
    }
    serde_json::from_slice(&result.stdout).map_err(error_text)
}
