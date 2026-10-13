//! Versioned testing interface. No language semantics or expected results live here.
pub use serde_json::Value;
use serde_json::json;
use std::{
    io::{Read, Write},
    sync::{Arc, Mutex},
};
pub const PROTOCOL_VERSION: u64 = 1;
pub const MAX_MESSAGE_BYTES: usize = 1_048_576;
pub const MAX_REQUEST_ID: u64 = 9_007_199_254_740_991;
pub trait Session {
    fn invoke(&mut self, invocation: Value) -> Result<Value, String>;
    fn validate(&mut self, _subject: &str, _value: Value) -> Result<Value, String> {
        Ok(json!({"kind":"unavailable","reason":"No production validator"}))
    }
    fn observe(&mut self, _entities: &[String]) -> Result<Value, String> {
        Err("No actual state observer".into())
    }
    fn close(&mut self) -> Result<(), String>;
}
#[derive(Default)]
struct Trace {
    events: Vec<Value>,
    calls: Vec<Value>,
    resources: Vec<Value>,
    bytes: usize,
    closed: bool,
    next_mock: u64,
}
#[derive(Clone)]
pub struct Context {
    state: Arc<Mutex<Trace>>,
}
impl Default for Context {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(Trace {
                next_mock: 1,
                ..Trace::default()
            })),
        }
    }
}
impl Context {
    fn record(&self, item: Value, kind: &str) -> Result<(), String> {
        let bytes = serde_json::to_vec(&item).map_err(|e| e.to_string())?.len();
        let mut trace = self.state.lock().map_err(|_| "Trace lock poisoned")?;
        if trace.closed {
            return Err("Tests session is closed".into());
        }
        if trace.bytes + bytes > MAX_MESSAGE_BYTES / 2
            || trace.events.len() + trace.calls.len() + trace.resources.len() >= 10000
        {
            return Err("Observation budget exceeded".into());
        }
        trace.bytes += bytes;
        match kind {
            "event" => trace.events.push(item),
            "call" => trace.calls.push(item),
            "resource" => trace.resources.push(item),
            _ => return Err("Invalid observation kind".into()),
        };
        Ok(())
    }
    pub fn emit(&self, name: &str, payload: Value) -> Result<(), String> {
        self.record(json!({"subject":name,"payload":payload}), "event")
    }
    pub fn record_call(&self, invocation: Value, observation: Value) -> Result<(), String> {
        self.record(
            json!({"invocation":invocation,"observation":observation}),
            "call",
        )
    }
    pub fn resource(&self, operation: &str, resource: &str, access: &str) -> Result<(), String> {
        self.record(
            json!({"operation":operation,"resource":resource,"access":access}),
            "resource",
        )
    }
    pub fn collaborator(&self, invocation: Value) -> Result<Value, String> {
        let request_id = {
            let mut trace = self.state.lock().map_err(|_| "Trace lock poisoned")?;
            if trace.closed {
                return Err("Session closed".into());
            }
            let id = trace.next_mock;
            trace.next_mock = trace.next_mock.checked_add(1).ok_or("Mock ID overflow")?;
            id
        };
        send(json!({"kind":"mock-call","request_id":request_id,"invocation":invocation}))?;
        let envelope = read()?.ok_or("Missing mock reply")?;
        if envelope["protocol_version"] != PROTOCOL_VERSION
            || envelope["request"]["kind"] != "mock-reply"
            || envelope["request"]["request_id"] != request_id
        {
            return Err("Invalid mock reply envelope".into());
        }
        envelope["request"]
            .get("observation")
            .cloned()
            .ok_or("Missing mock observation".into())
    }
    pub fn take(&self) -> Result<(Vec<Value>, Vec<Value>, Vec<Value>), String> {
        let mut trace = self.state.lock().map_err(|_| "Trace lock poisoned")?;
        trace.bytes = 0;
        Ok((
            std::mem::take(&mut trace.events),
            std::mem::take(&mut trace.calls),
            std::mem::take(&mut trace.resources),
        ))
    }
    pub fn finish(&self) -> Result<(), String> {
        self.state.lock().map_err(|_| "Trace lock poisoned")?.closed = true;
        Ok(())
    }
}
pub fn read() -> Result<Option<Value>, String> {
    let mut data = Vec::new();
    let mut stdin = std::io::stdin().lock();
    let mut byte = [0];
    loop {
        match stdin.read(&mut byte).map_err(|e| e.to_string())? {
            0 if data.is_empty() => return Ok(None),
            0 => return Err("Incomplete protocol line".into()),
            _ => {
                if byte[0] == b'\n' {
                    break;
                }
                if data.len() >= MAX_MESSAGE_BYTES {
                    return Err("Protocol message too large".into());
                }
                data.push(byte[0]);
            }
        }
    }
    serde_json::from_slice(&data)
        .map(Some)
        .map_err(|e| e.to_string())
}
pub fn send(message: Value) -> Result<(), String> {
    let packet = json!({"protocol_version":PROTOCOL_VERSION,"message":message});
    let bytes = serde_json::to_vec(&packet).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err("Protocol output exceeds budget".into());
    }
    let mut out = std::io::stdout().lock();
    out.write_all(&bytes)
        .and_then(|_| out.write_all(b"\n"))
        .and_then(|_| out.flush())
        .map_err(|e| e.to_string())
}
/// Catch only the actual application call; translation failures remain technical.
pub fn call_application<T, E: std::fmt::Display>(
    f: impl FnOnce() -> Result<T, E>,
) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(f))
        .map_err(|p| {
            format!(
                "Application panic: {}",
                p.downcast_ref::<String>()
                    .map(String::as_str)
                    .or_else(|| p.downcast_ref::<&str>().copied())
                    .unwrap_or("non-string panic")
            )
        })?
        .map_err(|e| e.to_string())
}
