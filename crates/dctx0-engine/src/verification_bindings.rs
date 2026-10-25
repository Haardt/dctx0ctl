//! Bounded, read-only binding input and planning. No discovery auto-acceptance.
use crate::{AccessMode, Error, Project, Result, VerificationSelection};
use dctx0_changes::HistoryReader;
use dctx0_core::{BindingDescription, BindingRequirements, VerificationBudgets};
use dctx0_verification::ProjectionFixtures;
use std::{fs, io::Read, path::Path};

const MAX_BINDING_BYTES: u64 = 524_288;

impl Project {
    pub fn verification_binding_requirements(
        &self,
        selection: VerificationSelection,
        fixtures: &ProjectionFixtures,
        budgets: VerificationBudgets,
    ) -> Result<BindingRequirements> {
        let plan = self.verification_plan(selection, fixtures, budgets)?;
        let snapshot = self
            .store(AccessMode::ReadOnly)?
            .load(&self.key(), &plan.snapshot)?;
        Ok(dctx0_verification::binding_requirements(
            &snapshot.ir,
            &plan,
        ))
    }
}

/// Read the explicit reviewed description. A file's existence is not a binding
/// confirmation, a native type proof or a successfully executed connection.
pub fn read_verification_bindings(
    application: &Path,
    relative: &str,
) -> Result<BindingDescription> {
    let root = fs::canonicalize(application)?;
    let (_, path) = crate::verification_setup::safe_file(&root, relative)?;
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_BINDING_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_BINDING_BYTES {
        return Err(Error::Invalid("Binding description exceeds 512 KiB".into()));
    }
    let description: BindingDescription = serde_json::from_slice(&bytes)?;
    description.validate().map_err(Error::Invalid)?;
    Ok(description)
}
