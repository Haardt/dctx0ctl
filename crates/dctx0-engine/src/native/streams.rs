use super::*;
use tokio::io::{AsyncRead, AsyncReadExt};
pub(super) async fn protocol(
    mut input: impl AsyncRead + Unpin,
    send: mpsc::Sender<Vec<u8>>,
    fault: watch::Sender<Option<NativeFailure>>,
    limit: usize,
) {
    let mut buffer = [0; 8192];
    let mut line = Vec::new();
    loop {
        let length = match input.read(&mut buffer).await {
            Ok(0) => {
                if !line.is_empty() {
                    fault.send_replace(Some(NativeFailure::new(
                        NativeFailureKind::Protocol,
                        "Incomplete JSON-Line at EOF",
                    )));
                }
                return;
            }
            Ok(n) => n,
            Err(error) => {
                fault.send_replace(Some(NativeFailure::io(error)));
                return;
            }
        };
        for byte in &buffer[..length] {
            if *byte == b'\n' {
                if send.send(std::mem::take(&mut line)).await.is_err() {
                    return;
                }
            } else {
                if line.len() >= limit {
                    fault.send_replace(Some(NativeFailure::new(
                        NativeFailureKind::MessageLimit,
                        "Worker message byte limit exceeded",
                    )));
                    return;
                }
                line.push(*byte);
            }
        }
    }
}
pub(super) async fn logs(
    mut input: impl AsyncRead + Unpin,
    logs: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    fault: watch::Sender<Option<NativeFailure>>,
    limit: usize,
) {
    let mut buffer = [0; 8192];
    loop {
        match input.read(&mut buffer).await {
            Ok(0) => return,
            Ok(length) => {
                let mut bytes = logs.lock().expect("log guard never spans awaits");
                let available = limit.saturating_sub(bytes.len());
                bytes.extend_from_slice(&buffer[..length.min(available)]);
                if length > available {
                    fault.send_replace(Some(NativeFailure::new(
                        NativeFailureKind::LogLimit,
                        "Application log byte limit exceeded",
                    )));
                    return;
                }
            }
            Err(error) => {
                fault.send_replace(Some(NativeFailure::io(error)));
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io,
        pin::Pin,
        task::{Context, Poll},
    };
    struct Broken;
    impl AsyncRead for Broken {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            _buffer: &mut tokio::io::ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            Poll::Ready(Err(io::Error::other("stream unavailable")))
        }
    }
    #[tokio::test]
    async fn stream_io_failures_and_closed_receivers_are_explicit() {
        let (send, mut receive) = mpsc::channel(1);
        let (signal, fault) = watch::channel(None);
        protocol(Broken, send, signal, 100).await;
        assert_eq!(fault.borrow().as_ref().unwrap().kind, NativeFailureKind::Io);
        assert!(receive.recv().await.is_none());
        let (send, receive) = mpsc::channel(1);
        drop(receive);
        let (signal, fault) = watch::channel(None);
        protocol(b"{}\n".as_slice(), send, signal, 100).await;
        assert!(fault.borrow().is_none());
        let (signal, fault) = watch::channel(None);
        let captured = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        logs(Broken, captured, signal, 100).await;
        assert_eq!(fault.borrow().as_ref().unwrap().kind, NativeFailureKind::Io);
    }
}
