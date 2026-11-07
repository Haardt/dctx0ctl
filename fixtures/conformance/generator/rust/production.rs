//! Independent production fixture; imports no dctx0 code.
use num_bigint::BigInt;
pub fn next(input:BigInt)->BigInt{input+1}
pub fn validate_count(value:&serde_json::Value)->bool{value.as_i64().is_some_and(|n|(1..=2).contains(&n))}
