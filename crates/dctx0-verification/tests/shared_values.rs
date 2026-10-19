use dctx0_core::VerificationValue;
use std::sync::Arc;

#[test]
fn cloned_native_text_shares_storage_and_preserves_canonical_wire_values() {
    let text: Arc<str> = "x".repeat(8 * 1024 * 1024).into();
    let value = VerificationValue::String(text.clone());
    let copies = vec![value.clone(); 100];
    for copy in &copies {
        let VerificationValue::String(shared) = copy else {
            panic!("string expected")
        };
        assert!(Arc::ptr_eq(shared, &text));
    }
    let encoded = serde_json::to_vec(&value).unwrap();
    let decoded: VerificationValue = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(decoded, value);
    assert_eq!(&encoded[..26], b"{\"kind\":\"string\",\"value\":\"");
}
