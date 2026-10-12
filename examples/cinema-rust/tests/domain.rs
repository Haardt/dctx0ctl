use cinema::domain::{candidate_order_winner, exact_integer, validate};
use serde_json::{Value, json};
#[test]
fn values_retain_exact_integers_and_explicit_unicode_trim_boundaries() {
    let value: Value = serde_json::from_str("900719925474099312345678901234567890").unwrap();
    assert_eq!(
        exact_integer(&value).unwrap().to_string(),
        "900719925474099312345678901234567890"
    );
    assert!(validate("ReleaseYear", &json!(1888)).is_ok());
    assert!(validate("ReleaseYear", &json!(1887)).is_err());
    assert!(validate("ReleaseYear", &json!(2101)).is_err());
    assert!(validate("CommentText", &json!("\u{feff}")).is_err());
    assert!(validate("CommentText", &json!("\u{0085}")).is_ok());
    assert!(validate("FilmId", &json!("00000000-0000-0000-0000-000000000000")).is_ok());
}
#[test]
fn winner_ignores_unknown_votes_and_uses_candidate_order_for_ties() {
    let candidates = vec![json!("a"), json!("b")];
    assert_eq!(
        candidate_order_winner(&[json!("b"), json!("a")], &candidates),
        json!("a")
    );
    assert_eq!(
        candidate_order_winner(&[json!("unknown")], &candidates),
        Value::Null
    );
    assert_eq!(candidate_order_winner(&[json!("a")], &[]), Value::Null);
}
