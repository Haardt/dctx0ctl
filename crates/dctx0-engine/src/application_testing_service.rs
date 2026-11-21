//! One bounded, private local assessment channel per diagnostic execution group.
use crate::{ApplicationTestingBundle, Error, Result};
use dctx0_core::WorkspaceIr;
#[cfg(unix)]
use dctx0_target_sdk::application_protocol::{self, AssessmentFrame};
use dctx0_verification::application_session::ApplicationAssessmentReport;
#[cfg(unix)]
use dctx0_verification::application_session::{
    ApplicationReply, ApplicationRequest, ApplicationSession,
};
#[cfg(unix)]
use std::time::Duration;
#[cfg(unix)]
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWriteExt, BufReader};

#[cfg(unix)]
#[derive(serde::Serialize)]
struct DecoderEvidenceFrame<'a> {
    kind: &'static str,
    context: u64,
    observation: &'a dctx0_verification::application_session::ApplicationObservation,
}

#[cfg(unix)]
async fn line(reader: &mut (impl AsyncBufRead + Unpin), limit: usize) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::new();
    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            return Ok(result);
        }
        let count = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |n| n + 1);
        if result.len() + count > limit {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Application assessment frame exceeds its budget",
            ));
        }
        result.extend_from_slice(&available[..count]);
        let complete = available[count - 1] == b'\n';
        reader.consume(count);
        if complete {
            return Ok(result);
        }
    }
}

#[cfg(unix)]
pub(crate) async fn serve(
    ir: &WorkspaceIr,
    bundle: &ApplicationTestingBundle,
    group: &str,
    endpoint: &str,
) -> Result<ApplicationAssessmentReport> {
    serve_cancellable(
        ir,
        bundle,
        group,
        endpoint,
        &crate::NativeCancellation::default(),
    )
    .await
}

#[cfg(unix)]
pub(crate) async fn serve_cancellable(
    ir: &WorkspaceIr,
    bundle: &ApplicationTestingBundle,
    group: &str,
    endpoint: &str,
    cancellation: &crate::NativeCancellation,
) -> Result<ApplicationAssessmentReport> {
    serve_cancellable_with_reuse(ir, bundle, group, endpoint, cancellation, vec![]).await
}

#[cfg(unix)]
pub(crate) async fn serve_cancellable_with_reuse(
    ir: &WorkspaceIr,
    bundle: &ApplicationTestingBundle,
    group: &str,
    endpoint: &str,
    cancellation: &crate::NativeCancellation,
    reused: Vec<dctx0_core::VerificationCaseRecord>,
) -> Result<ApplicationAssessmentReport> {
    let mut session =
        ApplicationSession::new_with_boundary(ir, &bundle.plan, group, bundle.boundary)
            .map_err(|issues| Error::Invalid(format!("Invalid assessment group: {issues:?}")))?
            .with_case_seconds(bundle.case_seconds)
            .with_lifecycle_inputs(bundle.lifecycle_inputs.clone())
            .with_reused_cases(reused)
            .map_err(|issues| Error::Invalid(format!("Invalid reused cases: {issues:?}")))?;
    let resources = &bundle
        .plan
        .configuration
        .groups
        .iter()
        .find(|g| g.id == group)
        .ok_or_else(|| Error::Invalid("Unknown group".into()))?
        .resources;
    let listener = listener(endpoint)?;
    let limit = resources.max_message_bytes as usize;
    let exchange = async {
        let (socket, _) = listener.0.accept().await?;
        let (read, mut write) = socket.into_split();
        let mut reader = BufReader::new(read);
        let mut previous = 0;
        let mut total = 0usize;
        let mut retained = std::collections::BTreeMap::<u64, usize>::new();
        let mut deadlines = std::collections::BTreeMap::<u64, tokio::time::Instant>::new();
        loop {
            let pending = deadlines
                .iter()
                .min_by_key(|(_, deadline)| **deadline)
                .map(|(context, deadline)| (*context, *deadline));
            let bytes = if let Some((context, deadline)) = pending {
                tokio::time::timeout_at(deadline, line(&mut reader, limit))
                    .await
                    .map_err(|_| {
                        Error::Invalid(format!("Application case {context} deadline exceeded"))
                    })??
            } else {
                line(&mut reader, limit).await?
            };
            if bytes.is_empty() {
                break;
            }
            total = total.saturating_add(bytes.len());
            if total > 268_435_456 {
                return Err(Error::Invalid("Application channel exceeds 256 MiB".into()));
            }
            let request: AssessmentFrame<ApplicationRequest> =
                application_protocol::decode(&bytes, limit)
                    .map_err(|e| Error::Invalid(e.to_string()))?;
            if request.id != previous + 1 {
                return Err(Error::Invalid(
                    "Application correlation IDs must be consecutive".into(),
                ));
            }
            previous = request.id;
            let mut account = |context, bytes| {
                if let Some(size) = retained.get_mut(&context) {
                    *size = size.saturating_add(bytes);
                    if *size > bundle.evidence_bytes {
                        return Err(Error::Invalid(format!(
                            "Application case {context} evidence budget exceeded"
                        )));
                    }
                }
                Ok(())
            };
            match &request.payload {
                ApplicationRequest::Observe { context, .. }
                | ApplicationRequest::Collaborator { context, .. } => {
                    account(*context, bytes.len())?
                }
                ApplicationRequest::CompleteDecoderBatch { cases, .. } => {
                    for case in cases {
                        if let Some(observation) = &case.observation {
                            let frame = AssessmentFrame::new(
                                request.id,
                                DecoderEvidenceFrame {
                                    kind: "observe",
                                    context: case.context,
                                    observation,
                                },
                            );
                            account(case.context, serde_json::to_vec(&frame)?.len() + 1)?;
                        }
                    }
                }
                _ => {}
            }
            let reply = session.handle(request.payload);
            let mut start = |context, seconds| {
                retained.insert(context, 0);
                deadlines.insert(
                    context,
                    tokio::time::Instant::now() + Duration::from_secs(seconds),
                );
            };
            match &reply {
                ApplicationReply::Case { context, .. } => start(*context, bundle.case_seconds),
                ApplicationReply::DecoderBatch { cases } => {
                    let seconds = bundle.case_seconds.saturating_mul(cases.len() as u64);
                    for case in cases {
                        start(case.context, seconds);
                    }
                }
                ApplicationReply::CaseClosed { context, .. } => {
                    deadlines.remove(context);
                    retained.remove(context);
                }
                ApplicationReply::DecoderBatchClosed { cases } => {
                    for case in cases {
                        deadlines.remove(&case.context);
                        retained.remove(&case.context);
                    }
                }
                _ => {}
            }
            let encoded =
                application_protocol::encode(&AssessmentFrame::new(request.id, reply), limit)
                    .map_err(|e| Error::Invalid(e.to_string()))?;
            let send = async {
                write.write_all(&encoded).await?;
                write.flush().await
            };
            if let Some(deadline) = deadlines.values().min().copied() {
                tokio::time::timeout_at(deadline, send)
                    .await
                    .map_err(|_| {
                        Error::Invalid("Application case reply deadline exceeded".into())
                    })??;
            } else {
                send.await?;
            }
        }
        Ok::<_, Error>(())
    };
    let result = tokio::select! {
        biased;
        result = tokio::time::timeout(Duration::from_secs(resources.timeout_seconds), exchange) => result,
        _ = cancellation.cancelled() => Ok(Err(Error::Invalid("Application assessment cancelled".into()))),
    };
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => session.record_transport_fault(error.to_string()),
        Err(_) => session.record_transport_fault("Application assessment channel timed out"),
    }
    Ok(session.report())
}

#[cfg(not(unix))]
pub(crate) async fn serve(
    _: &WorkspaceIr,
    _: &ApplicationTestingBundle,
    _: &str,
    _: &str,
) -> Result<ApplicationAssessmentReport> {
    Err(Error::Invalid(
        "Application assessment requires Unix socket support on this platform".into(),
    ))
}

#[cfg(unix)]
struct Listener(
    tokio::net::UnixListener,
    Option<(std::path::PathBuf, u64, u64)>,
);
#[cfg(unix)]
impl Drop for Listener {
    fn drop(&mut self) {
        use std::os::unix::fs::MetadataExt;
        if let Some((path, dev, ino)) = &self.1
            && std::fs::symlink_metadata(path).is_ok_and(|m| m.dev() == *dev && m.ino() == *ino)
        {
            let _ = std::fs::remove_file(path);
        }
    }
}
#[cfg(unix)]
fn listener(endpoint: &str) -> Result<Listener> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    if let Some(name) = endpoint.strip_prefix('@') {
        #[cfg(target_os = "linux")]
        {
            use std::os::linux::net::SocketAddrExt;
            let address = std::os::unix::net::SocketAddr::from_abstract_name(name)?;
            let socket = std::os::unix::net::UnixListener::bind_addr(&address)?;
            socket.set_nonblocking(true)?;
            return Ok(Listener(tokio::net::UnixListener::from_std(socket)?, None));
        }
        #[cfg(not(target_os = "linux"))]
        return Err(Error::Invalid(format!(
            "Abstract Unix sockets unavailable: {name}"
        )));
    }
    // bind rejects existing paths; never unlink an unrelated producer's endpoint.
    let socket = tokio::net::UnixListener::bind(endpoint)?;
    let meta = std::fs::symlink_metadata(endpoint)?;
    let listener = Listener(socket, Some((endpoint.into(), meta.dev(), meta.ino())));
    std::fs::set_permissions(endpoint, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}
