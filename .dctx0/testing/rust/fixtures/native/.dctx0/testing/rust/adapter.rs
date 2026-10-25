pub mod contract;
use contract::*;
use serde_json::json;
pub fn describe() -> Result<Value, TechnicalError> {
    Ok(
        json!({"protocol_version":1,"subjects":[{"subject":"Fixture.Api.increment","role":"operation","artifact":"src/lib.rs","entry_point":"increment"}],"artifacts":["src/lib.rs"],"capabilities":["sessions","invocation","events"],"representations":{"Int":"num_bigint::BigInt"}}),
    )
}
pub fn begin(_: &str, _: Context) -> Result<Box<dyn Session>, TechnicalError> {
    Ok(Box::new(Actual))
}
struct Actual;
impl Session for Actual {
    fn invoke(&mut self, invocation: Value) -> Result<Value, TechnicalError> {
        let input = invocation["arguments"][0]["value"]
            .as_str()
            .ok_or("Missing input")?
            .parse::<num_bigint::BigInt>()
            .map_err(|e| e.to_string())?;
        let result = dctx0_self_native_fixture::increment(input);
        Ok(json!({"kind":"returned","value":{"kind":"integer","value":result.to_string()}}))
    }
    fn close(&mut self) -> Result<(), TechnicalError> {
        Ok(())
    }
}
