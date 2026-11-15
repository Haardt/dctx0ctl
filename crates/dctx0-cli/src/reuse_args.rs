//! The same reuse policy applies to ordinary and complete native executions.
#[derive(Debug, Clone, clap::Args)]
pub struct ReuseArgs {
    /// Reuse compatible historical observations; off forces fresh execution.
    #[arg(long, default_value = "auto", value_parser = ["auto", "off", "require"])]
    reuse: String,
    /// Restrict the historical basis to this run.
    #[arg(long)]
    reuse_from: Option<String>,
    /// Additional deterministic scopes and snapshot/revision bindings.
    #[arg(long)]
    reuse_inputs: Option<String>,
}
impl ReuseArgs {
    pub fn options(self) -> Result<dctx0_engine::NativeReuseOptions, String> {
        let mode = self.reuse.parse()?;
        if mode == dctx0_engine::NativeReuseMode::Off && self.reuse_from.is_some() {
            return Err("--reuse=off conflicts with --reuse-from".into());
        }
        Ok(dctx0_engine::NativeReuseOptions {
            mode,
            inputs: self.reuse_inputs.unwrap_or_default(),
            parent: self.reuse_from,
        })
    }
}
