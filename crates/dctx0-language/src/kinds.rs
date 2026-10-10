use dctx0_core::StatementKind as K;

pub(crate) fn is_data_declaration(kind: K) -> bool {
    matches!(
        kind,
        K::Scalar | K::Value | K::Type | K::Failure | K::Entity | K::Message | K::Event
    )
}
pub(crate) fn is_effect_clause(kind: K) -> bool {
    matches!(kind, K::Read | K::Write | K::Call | K::Emit | K::Observe)
}

/// Observations can refer to modeled operations or integration boundaries.
pub(crate) fn is_observable_target(kind: K) -> bool {
    matches!(kind, K::Operation | K::Port | K::External)
}
/// Policy requirements point to evidence-bearing declarations.
pub(crate) fn is_verification_target(kind: K) -> bool {
    matches!(kind, K::Proof | K::Test | K::Review)
}
