use clap::{Args, Subcommand, ValueEnum};
use dctx0_engine::{Editor, EditorInstallRequest, activate_vscode, install_editor};
use std::{io::Write, path::PathBuf};
#[derive(Args)]
pub(crate) struct EditorArgs {
    #[command(subcommand)]
    action: Action,
}
#[derive(Subcommand)]
enum Action {
    /// Prepare a local VSIX or Zed dev extension; optionally install the VSIX in VS Code.
    Install {
        #[arg(value_enum)]
        editor: Kind,
        #[arg(long, default_value = ".dctx0/editors")]
        output: PathBuf,
        #[arg(long)]
        force: bool,
        #[arg(long)]
        dry_run: bool,
        /// Invoke `code --install-extension` after preparing the VSIX (VS Code only).
        #[arg(long)]
        activate: bool,
    },
}
#[derive(Clone, Copy, ValueEnum)]
enum Kind {
    Vscode,
    Zed,
}
pub(crate) fn run(args: EditorArgs, json: bool, output: &mut dyn Write) -> Result<(), String> {
    let Action::Install {
        editor,
        output: destination,
        force,
        dry_run,
        activate,
    } = args.action;
    let editor = match editor {
        Kind::Vscode => Editor::Vscode,
        Kind::Zed => Editor::Zed,
    };
    if activate && editor != Editor::Vscode {
        return Err(
            "Zed activation requires its ‘install dev extension’ UI; omit --activate".into(),
        );
    }
    let mut report = install_editor(&EditorInstallRequest {
        editor,
        output: destination,
        executable: std::env::current_exe().map_err(crate::error_message)?,
        force,
        dry_run,
    })
    .map_err(crate::error_message)?;
    if activate && !dry_run {
        activate_vscode(&report.artifact, force).map_err(crate::error_message)?;
        report.status = "installed-in-vscode".into();
    }
    if json {
        crate::render::json(output, &report)
    } else {
        writeln!(
            output,
            "{}: {}\n{}",
            report.status,
            report.artifact.display(),
            report.instructions
        )
        .map_err(crate::error_message)
    }
}
