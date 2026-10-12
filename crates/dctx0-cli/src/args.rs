use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    name = "dctx0",
    version,
    about = "Compile specs, inspect changes, and explicitly rename symbols"
)]
pub(crate) struct Cli {
    #[arg(long, global = true)]
    pub(crate) db: Option<PathBuf>,
    #[arg(long, global = true)]
    pub(crate) json: bool,
    #[command(subcommand)]
    pub(crate) command: Command,
}
#[derive(Subcommand)]
pub(crate) enum Command {
    /// Run the read-only language server over stdin/stdout.
    Lsp {
        #[arg(long)]
        project: Option<PathBuf>,
        #[arg(long)]
        stdio: bool,
    },
    /// Package or install local editor extensions.
    Editor(crate::editors::EditorArgs),
    /// List or install the bundled agent skills.
    Skills(crate::skills::SkillArgs),
    /// Install bundled skills (compatible shorthand for skills install).
    Install(crate::skills::SkillInstallArgs),
    Verify(crate::verification::VerifyArgs),
    Tasks(Box<crate::tasks::TaskArgs>),
    Profile(crate::profiles::ProfileArgs),
    Parse {
        input: PathBuf,
    },
    BenchParse {
        input: PathBuf,
        #[arg(default_value = "1")]
        iterations: usize,
    },
    BenchCompile {
        input: PathBuf,
        #[arg(default_value = "1")]
        iterations: usize,
    },
    BenchProfiles {
        input: PathBuf,
        target: String,
        #[arg(value_enum)]
        phase: crate::profile_benchmark::ProfilePhase,
        #[arg(default_value = "1")]
        iterations: usize,
    },
    Bench {
        input: PathBuf,
        #[arg(value_enum)]
        phase: Phase,
        #[arg(default_value = "1")]
        iterations: usize,
    },
    Check {
        input: PathBuf,
    },
    History {
        input: PathBuf,
    },
    Diff {
        input: PathBuf,
        before: String,
        after: String,
    },
    Impact {
        input: PathBuf,
        symbol: String,
        #[arg(long, default_value = "latest")]
        snapshot: String,
    },
    Rename {
        input: PathBuf,
        symbol: String,
        new_name: String,
        #[arg(long)]
        dry_run: bool,
    },
}
#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum Phase {
    VerificationHarness,
    VerificationDiscover,
    VerificationInit,
    VerificationHistoryPublish,
    VerificationHistoryRead,
    VerificationProject,
    VerificationGenerate,
    VerificationEvaluateUnbound,
    VerificationAssessUnbound,
    Compile,
    Graph,
    Snapshot,
    Diff,
    DiffChanged,
    Persist,
    Check,
    CheckChanged,
    TasksPlan,
    TasksReconcile,
    TasksPublish,
    TasksList,
    TasksContextMd,
    TasksContextJson,
    TasksSlice,
    TasksClaim,
}
