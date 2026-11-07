//! Child-only fixture environments. Provisioning and ownership stay authored.
use std::{
    collections::BTreeMap,
    process::{Command, Output},
};
pub struct Fixture {
    pub environment: BTreeMap<String, Option<String>>,
    pub cleanup: Option<Box<dyn FnOnce()>>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(cleanup) = self.cleanup.take() {
            cleanup()
        }
    }
}
pub fn configured_fixture(key: &str) -> Result<String, String> {
    std::env::var(key)
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("fixture prerequisite {key} is not configured"))
}
pub fn plan_command(
    cli: &str,
    project: &std::path::Path,
    args: &[&str],
    environment: &BTreeMap<String, Option<String>>,
) -> Result<Command, String> {
    let mut command = Command::new(cli);
    command.args(args).current_dir(project);
    for (key, value) in environment {
        if key.is_empty() || key.contains(['=', '\0']) {
            return Err("invalid fixture environment key".into());
        }
        match value {
            Some(v) => {
                if v.contains('\0') {
                    return Err(format!("invalid fixture environment value for {key}"));
                }
                command.env(key, v);
            }
            None => {
                command.env_remove(key);
            }
        }
    }
    Ok(command)
}
pub fn run_plan(args: &[&str], fixture: &Fixture) -> Result<(), String> {
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let cli = std::env::var("DCTX0_TEST_CLI").unwrap_or_else(|_| "dctx0".into());
    let Output { status, stdout, .. } = plan_command(&cli, &project, args, &fixture.environment)?
        .output()
        .map_err(|_| "central verification process unavailable")?;
    let report: serde_json::Value = serde_json::from_slice(&stdout).map_err(
        |_| "central verification did not return a report; inspect bindings test directly",
    )?;
    if !status.success()
        || report["published"] != false
        || report["run"]["result"]["status"] != "PASS"
    {
        return Err(
            "central verification failed or is incomplete; inspect bindings test directly".into(),
        );
    }
    Ok(())
}
