//! Task CLI adapters keep arguments, orchestration and rendering separate.
mod args;
mod commands;
mod render;
pub(crate) use args::TaskArgs;
pub(crate) use commands::run;
