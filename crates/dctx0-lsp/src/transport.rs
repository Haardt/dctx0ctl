//! Bounded stdio framing. The blocking stdin thread ends at EOF/process exit.
use crossbeam_channel::{Receiver, bounded};
use lsp_server::{Connection, Message};
use std::io::{BufRead, BufReader, Read};
const MAX_MESSAGE: usize = 4 * 1024 * 1024;
const MAX_HEADER: usize = 8192;
pub(crate) fn stdio() -> (
    Connection,
    std::thread::JoinHandle<Result<(), String>>,
    Receiver<String>,
) {
    let (incoming, receiver) = bounded(16);
    let (sender, outgoing) = bounded::<Message>(16);
    let (errors, error_receiver) = bounded(1);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(std::io::stdin());
        loop {
            match read(&mut reader) {
                Ok(Some(message)) => {
                    if incoming.send(message).is_err() {
                        break;
                    }
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = errors.send(error.to_string());
                    break;
                }
            }
        }
    });
    let writer = std::thread::spawn(move || {
        let stdout = std::io::stdout();
        let mut writer = stdout.lock();
        for message in outgoing {
            message.write(&mut writer).map_err(|e| e.to_string())?;
        }
        Ok(())
    });
    (Connection { sender, receiver }, writer, error_receiver)
}
fn read(reader: &mut impl BufRead) -> std::io::Result<Option<Message>> {
    use std::io::{Error, ErrorKind};
    let mut length = None;
    let mut total = 0;
    loop {
        let mut header = Vec::new();
        let count = reader
            .take((MAX_HEADER + 1 - total) as u64)
            .read_until(b'\n', &mut header)?;
        if count == 0 {
            return if total == 0 {
                Ok(None)
            } else {
                Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "Incomplete LSP header",
                ))
            };
        }
        total += count;
        if total > MAX_HEADER {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "LSP header exceeds 8 KiB",
            ));
        }
        if header == b"\r\n" {
            break;
        }
        let text =
            std::str::from_utf8(&header).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
        if let Some((key, value)) = text.trim_end().split_once(':') {
            if key.eq_ignore_ascii_case("Content-Length") {
                if length.is_some() {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Duplicate Content-Length",
                    ));
                }
                length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|e| Error::new(ErrorKind::InvalidData, e))?,
                );
            }
        } else {
            return Err(Error::new(ErrorKind::InvalidData, "Malformed LSP header"));
        }
    }
    let length = length.filter(|l| *l <= MAX_MESSAGE).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidData,
            "Missing or oversized Content-Length",
        )
    })?;
    let mut body = vec![0; length];
    reader.read_exact(&mut body)?;
    let value: serde_json::Value =
        serde_json::from_slice(&body).map_err(|e| Error::new(ErrorKind::InvalidData, e))?;
    if value["jsonrpc"] != "2.0" {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "LSP requires JSON-RPC 2.0",
        ));
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| Error::new(ErrorKind::InvalidData, e))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn framing_rejects_truncated_invalid_duplicate_and_oversized_messages() {
        assert!(read(&mut &b""[..]).unwrap().is_none());
        let body = br#"{"jsonrpc":"2.0","method":"exit"}"#;
        let packet = format!(
            "Content-Length: {}\r\n\r\n{}",
            body.len(),
            std::str::from_utf8(body).unwrap()
        );
        assert!(matches!(
            read(&mut packet.as_bytes()).unwrap(),
            Some(Message::Notification(_))
        ));
        for bad in [
            b"invalid\r\n\r\n".as_slice(),
            b"Content-Length: 1\r\nContent-Length: 1\r\n\r\nx",
            b"Content-Length: 4194305\r\n\r\n",
            b"Content-Length: 9\r\n\r\nx",
            b"Content-Length: nope\r\n\r\n",
            b"Content-Length: 1\r\n\r\nx",
            b"Content-Length: 1\r\n",
            b"Content-Type: x\r\n\r\n",
        ] {
            assert!(read(&mut &bad[..]).is_err());
        }
        assert!(read(&mut vec![b'x'; 8193].as_slice()).is_err());
    }
}
