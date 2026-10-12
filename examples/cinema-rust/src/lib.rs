//! Cinema application; independent of verification tooling.
pub mod application;
pub mod domain;
pub mod httpapi;
pub mod persistence;

#[cfg(feature = "axum-http")]
pub mod axumapi;
