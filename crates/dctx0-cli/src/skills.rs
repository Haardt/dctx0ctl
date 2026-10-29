//! Skill selection and presentation; installation effects belong to the engine.
use clap::{Args, Subcommand};
use dctx0_engine::{
    SKILL_AGENTS, SKILL_NAMES, SkillInstallRequest, SkillLocations, install_skills, skill_catalog,
};
use std::{collections::BTreeSet, io::Write, path::PathBuf};
#[derive(Args)]
pub(crate) struct SkillArgs {
    #[command(subcommand)]
    action: SkillAction,
}
#[derive(Subcommand)]
enum SkillAction {
    List,
    Install(SkillInstallArgs),
}
#[derive(Args)]
pub(crate) struct SkillInstallArgs {
    #[arg(long, default_value = ".")]
    project: PathBuf,
    #[arg(long,value_delimiter=',',default_value="agents",value_parser=["agents","claude","codex","copilot","cursor","gemini","opencode","all"])]
    agent: Vec<String>,
    #[arg(long,value_delimiter=',',value_parser=["dctx0","dctx0-language","dctx0-implementation","dctx0-config","dctx0-delivery"])]
    skill: Vec<String>,
    #[arg(long)]
    global: bool,
    #[arg(long)]
    force: bool,
    #[arg(long)]
    dry_run: bool,
}
pub(crate) fn install(
    args: SkillInstallArgs,
    json: bool,
    output: &mut dyn Write,
) -> Result<(), String> {
    let agents = if args.agent.iter().any(|a| a == "all") {
        SKILL_AGENTS.iter().map(|s| (*s).into()).collect()
    } else {
        args.agent.into_iter().collect::<BTreeSet<_>>()
    };
    let skills = if args.skill.is_empty() {
        SKILL_NAMES.iter().map(|s| (*s).into()).collect()
    } else {
        args.skill.into_iter().collect()
    };
    let report = install_skills(
        &SkillInstallRequest {
            project: args.project,
            agents,
            skills,
            global: args.global,
            force: args.force,
            dry_run: args.dry_run,
        },
        &SkillLocations::from_environment(),
    )
    .map_err(crate::error_message)?;
    if json {
        crate::render::json(output, &report)
    } else {
        for item in report.installations {
            writeln!(
                output,
                "{}: {:?} → {} ({})",
                item.skill,
                item.status,
                item.directory.display(),
                item.agents.join(", ")
            )
            .map_err(crate::error_message)?;
        }
        for warning in report.warnings {
            writeln!(output, "Warning: {warning}").map_err(crate::error_message)?;
        }
        Ok(())
    }
}
pub(crate) fn run(args: SkillArgs, json: bool, output: &mut dyn Write) -> Result<(), String> {
    match args.action {
        SkillAction::Install(args) => install(args, json, output),
        SkillAction::List => {
            let catalog = skill_catalog();
            if json {
                crate::render::json(output, &catalog)
            } else {
                for entry in catalog {
                    writeln!(
                        output,
                        "{} v{} ({} files)",
                        entry.name,
                        entry.version,
                        entry.files.len()
                    )
                    .map_err(crate::error_message)?;
                }
                Ok(())
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_and_default_preview_are_available_to_embedded_cli_callers() {
        let mut output = Vec::new();
        run(
            SkillArgs {
                action: SkillAction::List,
            },
            false,
            &mut output,
        )
        .unwrap();
        assert!(
            String::from_utf8(output)
                .unwrap()
                .contains("dctx0-language")
        );
        let root =
            std::env::temp_dir().join(format!("dctx0-skills-embedded-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let mut output = Vec::new();
        install(
            SkillInstallArgs {
                project: root.clone(),
                agent: vec!["all".into()],
                skill: vec![],
                global: false,
                force: false,
                dry_run: true,
            },
            true,
            &mut output,
        )
        .unwrap();
        let report: serde_json::Value = serde_json::from_slice(&output).unwrap();
        assert_eq!(
            report["installations"].as_array().unwrap().len(),
            dctx0_engine::SKILL_NAMES.len() * 6
        );
        assert!(!root.join(".agents").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}
