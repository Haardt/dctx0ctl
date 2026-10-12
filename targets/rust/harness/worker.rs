//! Generated Rust transport. Observations are assessed by the central verifier.
use dctx0_project_adapter::{self as adapter, contract::*};
use serde_json::json;
fn field<'a>(value: &'a Value, name: &str) -> Result<&'a str, String> {
    value[name]
        .as_str()
        .ok_or_else(|| format!("Missing {name}"))
}
fn metrics() -> Value {
    let peak = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|line| {
                line.strip_prefix("VmHWM:")
                    .and_then(|x| x.split_whitespace().next())
                    .and_then(|x| x.parse::<u64>().ok())
                    .and_then(|n| n.checked_mul(1024))
            })
        });
    json!({"kind":"measured","peak_rss_bytes":peak,"runtime_version":include_str!("build-version.txt").trim(),"platform":std::env::consts::OS})
}
fn run() -> Result<(), String> {
    let subjects: Vec<String> = serde_json::from_str(__SUBJECTS__).map_err(|e| e.to_string())?;
    let mut session: Option<Box<dyn Session>> = None;
    let mut context = Context::default();
    let mut last_id = 0;
    loop {
        let Some(envelope) = read()? else {
            if let Some(mut session) = session.take() {
                session.close()?;
                context.finish()?;
            }
            return Err("Worker input closed without shutdown".into());
        };
        let id = envelope["id"]
            .as_u64()
            .filter(|n| *n > last_id && *n <= MAX_REQUEST_ID)
            .ok_or("Invalid or repeated request ID")?;
        last_id = id;
        if envelope["protocol_version"] != PROTOCOL_VERSION {
            return Err("Unsupported protocol version".into());
        }
        let request = &envelope["request"];
        let kind = field(request, "kind")?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
            || -> Result<Value, String> {
                match kind {
                    "metrics" => Ok(metrics()),
                    "describe" => {
                        Ok(json!({"kind":"described","description":adapter::describe()?}))
                    }
                    "begin" => {
                        if session.is_some() {
                            return Err("Session already active".into());
                        }
                        context = Context::default();
                        let case = field(request, "case_id")?;
                        session = Some(adapter::begin(case, context.clone())?);
                        Ok(json!({"kind":"begun","case_id":case}))
                    }
                    "invoke" => {
                        let invocation = request
                            .get("invocation")
                            .cloned()
                            .ok_or("Missing invocation")?;
                        let subject = field(&invocation, "subject")?;
                        if !subjects
                            .iter()
                            .any(|s| subject == s || subject.starts_with(&format!("{s}.")))
                        {
                            return Err("Invocation outside plan subjects".into());
                        }
                        let observation = session
                            .as_mut()
                            .ok_or("No active session")?
                            .invoke(invocation)?;
                        let (events, calls, resources) = context.take()?;
                        let mut reply =
                            json!({"kind":"invoked","observation":observation,"events":events});
                        let description = adapter::describe()?;
                        if description["capabilities"]
                            .as_array()
                            .is_some_and(|c| c.contains(&json!("call-observation")))
                        {
                            reply["calls"] = json!(calls);
                        }
                        if description["capabilities"]
                            .as_array()
                            .is_some_and(|c| c.contains(&json!("resource-observation")))
                        {
                            reply["resources"] = json!(resources);
                        }
                        Ok(reply)
                    }
                    "validate" => {
                        let subject = field(request, "subject")?;
                        let value = request.get("value").cloned().ok_or("Missing value")?;
                        Ok(
                            json!({"kind":"value-checked","outcome":session.as_mut().ok_or("No active session")?.validate(subject,value)?}),
                        )
                    }
                    "observe" => {
                        let entities =
                            serde_json::from_value::<Vec<String>>(request["entities"].clone())
                                .map_err(|e| e.to_string())?;
                        Ok(
                            json!({"kind":"observed","states":session.as_mut().ok_or("No active session")?.observe(&entities)?}),
                        )
                    }
                    "end" => {
                        let mut current = session.take().ok_or("No active session")?;
                        let closed = current.close();
                        context.finish()?;
                        closed?;
                        Ok(json!({"kind":"ended"}))
                    }
                    "shutdown" => {
                        if let Some(mut current) = session.take() {
                            let closed = current.close();
                            context.finish()?;
                            closed?;
                        }
                        Ok(json!({"kind":"bye"}))
                    }
                    "resolve-dependency" => Ok(
                        json!({"kind":"dependency-unresolved","reason":"Cargo dependency closure is recorded at build time"}),
                    ),
                    _ => Err("Unsupported request".into()),
                }
            },
        ));
        match result {
            Ok(Ok(reply)) => send(json!({"kind":"reply","id":id,"reply":reply}))?,
            Ok(Err(message)) => send(
                json!({"kind":"technical-error","id":id,"category":"adapter","message":message.chars().take(4096).collect::<String>()}),
            )?,
            Err(_) => send(
                json!({"kind":"technical-error","id":id,"category":"adapter","message":"Adapter panicked outside application boundary"}),
            )?,
        }
        if kind == "shutdown" {
            return Ok(());
        }
    }
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Rust worker: {error}");
        std::process::exit(1);
    }
}
