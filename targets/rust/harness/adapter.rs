//! Reviewed testing-only adapter. Add production dependencies to Cargo.toml,
//! implement actual entry points, then explicitly prepare Cargo.lock.
pub mod contract;
use contract::*;
pub fn describe() -> Result<Value, TechnicalError> {
    Err(TechnicalError::adapter(
        "Project adapter is not implemented",
    ))
}
pub fn begin(_case: &str, _context: Context) -> Result<Box<dyn Session>, TechnicalError> {
    Err(TechnicalError::adapter(
        "Project adapter is not implemented",
    ))
}
