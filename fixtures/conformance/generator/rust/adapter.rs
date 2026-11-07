pub mod contract;
mod wire_codecs;
#[cfg(test)] mod contract_plan_tests;
use contract::*;
use serde_json::json;
pub fn describe()->Result<Value,TechnicalError>{Ok(json!({"protocol_version":1,"subjects":[{"subject":"W.Api.next","role":"operation","artifact":"src/lib.rs","entry_point":"fixture::next"},{"subject":"W.Count","role":"validator","artifact":"src/lib.rs","entry_point":"fixture::validate_count"}],"artifacts":["src/lib.rs"],"capabilities":["sessions","invocation","events","values"],"representations":{"Int":"num_bigint::BigInt"}}))}
pub fn begin(_: &str,_:Context)->Result<Box<dyn Session>,TechnicalError>{Ok(Box::new(Actual))}
// Foreign option/variant/failure inputs retain an authored tagged envelope.
struct Actual;
impl Session for Actual {
 fn invoke(&mut self,invocation:Value)->Result<Value,TechnicalError>{let input=invocation["arguments"][0]["value"].as_str().ok_or("Missing input")?.parse::<num_bigint::BigInt>().map_err(|e|TechnicalError::adapter(e.to_string()))?;let result=fixture::next(input);Ok(json!({"kind":"returned","value":{"kind":"integer","value":result.to_string()}}))}
 fn validate(&mut self,subject:&str,value:Value)->Result<Value,TechnicalError>{let native=if matches!(value["kind"].as_str(),Some("option"|"variant"|"failure")){value.clone()}else{wire_codecs::decode_input(subject,&value).map_err(TechnicalError::adapter)?};if fixture::validate_count(&native){Ok(json!({"kind":"accepted","value":value}))}else{Ok(json!({"kind":"rejected","reason":"Production count parser rejected input"}))}}
 fn close(&mut self)->Result<(),TechnicalError>{Ok(())}
}
