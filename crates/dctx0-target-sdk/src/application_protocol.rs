//! Application-test assessment framing, independent of the adapter worker protocol.
//! Payloads contain observations and evaluator directives; they confer no task authority.
use crate::{Error, MAX_MESSAGE_BYTES, MAX_REQUEST_ID};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

pub const APPLICATION_PROTOCOL_VERSION: u32 = 2;
pub const GO_APPLICATION_LIBRARY_VERSION: &str = "0.2.0";

/// A target-owned, testing-only library embedded in the shipped CLI. Publishing
/// these artifacts neither edits production dependencies nor authorizes tests.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApplicationTestLibrary {
    pub target: String,
    pub version: String,
    pub protocol_version: u32,
    pub artifacts: Vec<crate::Artifact>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AssessmentFrame<T> {
    pub protocol_version: u32,
    pub id: u64,
    pub payload: T,
}

impl<T> AssessmentFrame<T> {
    pub fn new(id: u64, payload: T) -> Self {
        Self {
            protocol_version: APPLICATION_PROTOCOL_VERSION,
            id,
            payload,
        }
    }
    fn validate(&self) -> Result<(), Error> {
        if self.protocol_version != APPLICATION_PROTOCOL_VERSION {
            return Err(Error::Protocol(
                "Unsupported application assessment protocol".into(),
            ));
        }
        if self.id == 0 || self.id > MAX_REQUEST_ID {
            return Err(Error::Protocol(
                "Application assessment correlation ID is outside its range".into(),
            ));
        }
        Ok(())
    }
}

fn budget(limit: usize) -> Result<(), Error> {
    if limit == 0 || limit > MAX_MESSAGE_BYTES {
        return Err(Error::Protocol(
            "Invalid application assessment frame budget".into(),
        ));
    }
    Ok(())
}

/// A single bounded JSON line. Raw decoder inputs are octet arrays inside the
/// payload, so invalid UTF-8 and invalid JSON reach the production decoder intact.
pub fn encode<T: Serialize>(frame: &AssessmentFrame<T>, limit: usize) -> Result<Vec<u8>, Error> {
    budget(limit)?;
    frame.validate()?;
    let mut bytes =
        serde_json::to_vec(frame).map_err(|error| Error::Protocol(error.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > limit {
        return Err(Error::Protocol(
            "Application assessment frame exceeds its budget".into(),
        ));
    }
    Ok(bytes)
}

pub fn decode<T: DeserializeOwned>(
    bytes: &[u8],
    limit: usize,
) -> Result<AssessmentFrame<T>, Error> {
    budget(limit)?;
    if bytes.len() > limit {
        return Err(Error::Protocol(
            "Application assessment frame exceeds its budget".into(),
        ));
    }
    let frame: AssessmentFrame<T> =
        serde_json::from_slice(bytes).map_err(|error| Error::Protocol(error.to_string()))?;
    frame.validate()?;
    Ok(frame)
}
