//! SQLite task adapter: domain rules stay in dctx0-tasks.
use super::*;
use crate::Error;
use dctx0_tasks::*;
use std::collections::{BTreeMap, BTreeSet};
mod actions;
mod native_proof;
pub use actions::{TaskDelivery, TaskDeliveryPublication, TaskDeliveryPublicationReceipt};
mod profiles;
mod publish;
mod read;
mod schema;
use read::{event, latest_check};
pub(super) use read::{head, session, watermark};

pub(super) use schema::{MIGRATION, PROFILE_MIGRATION};
fn invalid(message: impl Into<String>) -> Error {
    dctx0_tasks::Error::Invalid(message.into()).into()
}
fn conflict(message: impl Into<String>) -> Error {
    dctx0_tasks::Error::Conflict(message.into()).into()
}

#[cfg(test)]
mod tests;
