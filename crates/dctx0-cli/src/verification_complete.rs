//! Collective memory guard for complete native acceptance, without Python.
use std::{
    fs,
    process::{Command, Stdio},
};

fn memory_budget(text: &str) -> Result<u64, String> {
    let fields = text
        .lines()
        .find(|line| line.starts_with("MemAvailable:"))
        .ok_or_else(|| "MemAvailable is unavailable; refusing unbounded acceptance".to_string())?
        .split_whitespace()
        .collect::<Vec<_>>();
    if fields.len() != 3 || fields[2] != "kB" {
        return Err("Invalid MemAvailable unit".into());
    }
    fields[1]
        .parse::<u64>()
        .ok()
        .filter(|value| *value > 0)
        .and_then(|value| value.checked_mul(1024))
        .map(|value| value / 3 * 2)
        .ok_or_else(|| "Invalid MemAvailable value".into())
}

pub(crate) fn memory_guard() -> Result<Option<bool>, String> {
    if std::env::var("DCTX0_COMPLETE_MEMORY_GUARDED").as_deref() == Ok("1") {
        return Ok(None);
    }
    if std::env::var("DCTX0_MEMORY_GUARD").as_deref() == Ok("0")
        || !cfg!(target_os = "linux")
        || !systemd_available()
    {
        eprintln!("Memory guard disabled or unavailable; running without a collective RAM cap.");
        return Ok(None);
    }
    let budget =
        memory_budget(&fs::read_to_string("/proc/meminfo").map_err(crate::error_message)?)?;
    eprintln!(
        "Memory guard: {budget} bytes for complete verification and descendants; swap disabled."
    );
    let status = Command::new("systemd-run")
        .args(["--user", "--scope", "--quiet", "--property"])
        .arg(format!("MemoryMax={budget}"))
        .args(["--property", "MemorySwapMax=0", "--"])
        .arg(std::env::current_exe().map_err(crate::error_message)?)
        .args(std::env::args_os().skip(1))
        .env("DCTX0_COMPLETE_MEMORY_GUARDED", "1")
        .status()
        .map_err(crate::error_message)?;
    Ok(Some(status.success()))
}

fn systemd_available() -> bool {
    Command::new("systemd-run")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
        && Command::new("systemctl")
            .args(["--user", "show-environment"])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn memory_budget_requires_positive_available_kib_and_rejects_overflow() {
        assert_eq!(
            memory_budget("MemFree: 5 kB\nMemAvailable: 3000 kB\n").unwrap(),
            2_048_000
        );
        for invalid in [
            "",
            "MemAvailable: 0 kB",
            "MemAvailable: 3 MB",
            "MemAvailable: x kB",
            "MemAvailable: 18446744073709551615 kB",
            "MemAvailable: 1 kB extra",
        ] {
            assert!(memory_budget(invalid).is_err(), "{invalid}");
        }
    }
}
