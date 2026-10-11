//! Identity read by the managing process, independent of worker claims.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRuntimeIdentity {
    pub schema_version: u32,
    pub executable_hash: String,
    pub executable_path: String,
    pub executable_bytes: u64,
    pub initial_environment_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct VerifierIdentity {
    pub schema_version: u32,
    pub executable_hash: String,
    pub executable_bytes: u64,
}
static VERIFIER_IDENTITY: std::sync::OnceLock<VerifierIdentity> = std::sync::OnceLock::new();
impl VerifierIdentity {
    pub(crate) fn capture() -> Result<Self, NativeFailure> {
        if let Some(identity) = VERIFIER_IDENTITY.get() {
            return Ok(identity.clone());
        }
        let file = std::fs::File::open("/proc/self/exe").map_err(NativeFailure::io)?;
        let (executable_hash, executable_bytes) = hash_bounded(file, 268_435_456)?;
        let identity = Self {
            schema_version: 1,
            executable_hash,
            executable_bytes,
        };
        // The running Linux executable image is fixed for this process. Cache
        // only a successful identity; transient read failures may be retried.
        let _ = VERIFIER_IDENTITY.set(identity.clone());
        Ok(identity)
    }
}

fn hash_bounded(mut input: impl Read, limit: u64) -> Result<(String, u64), NativeFailure> {
    let mut hasher = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 16384];
    loop {
        let count = input.read(&mut buffer).map_err(NativeFailure::io)?;
        if count == 0 {
            return Ok((format!("{:x}", hasher.finalize()), size));
        }
        size += count as u64;
        if size > limit {
            return Err(NativeFailure::new(
                NativeFailureKind::Technical,
                "Runtime identity exceeds its capture limit",
            ));
        }
        hasher.update(&buffer[..count]);
    }
}

impl NativeWorker {
    /// Linux procfs binds the executable actually running and its initial
    /// environment. Environment names and values are not persisted.
    pub fn runtime_identity(&self) -> Result<NativeRuntimeIdentity, NativeFailure> {
        if self.child.is_none() || self.input.is_none() {
            return Err(NativeFailure::new(
                NativeFailureKind::WorkerExit,
                "Runtime identity requires a live managed worker",
            ));
        }
        if self.next_id <= 1 {
            return Err(NativeFailure::new(
                NativeFailureKind::Technical,
                "Runtime identity requires an acknowledged worker request",
            ));
        }
        let executable =
            std::fs::File::open(format!("/proc/{}/exe", self.pid)).map_err(NativeFailure::io)?;
        let (executable_hash, executable_bytes) = hash_bounded(executable, 268_435_456)?;
        let environment = std::fs::File::open(format!("/proc/{}/environ", self.pid))
            .map_err(NativeFailure::io)?;
        let initial_environment_hash = environment_hash(environment)?;
        let executable_path = std::fs::read_link(format!("/proc/{}/exe", self.pid))
            .map_err(NativeFailure::io)?
            .to_string_lossy()
            .into_owned();
        Ok(NativeRuntimeIdentity {
            schema_version: 1,
            executable_hash,
            executable_path,
            executable_bytes,
            initial_environment_hash,
        })
    }
}

fn environment_hash(input: impl Read) -> Result<String, NativeFailure> {
    let mut bytes = Vec::new();
    input
        .take(1_048_577)
        .read_to_end(&mut bytes)
        .map_err(NativeFailure::io)?;
    if bytes.len() > 1_048_576 {
        return Err(NativeFailure::new(
            NativeFailureKind::Technical,
            "Runtime environment exceeds its capture limit",
        ));
    }
    let mut entries = bytes
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    entries.sort();
    let mut hash = Sha256::new();
    for entry in entries {
        hash.update(entry);
        hash.update([0]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl NativeRuntimeIdentity {
    pub fn still_current(&self) -> Result<bool, NativeFailure> {
        if self.schema_version != 1 || self.executable_path.is_empty() {
            return Ok(false);
        }
        let file = std::fs::File::open(&self.executable_path).map_err(NativeFailure::io)?;
        let (hash, bytes) = hash_bounded(file, 268_435_456)?;
        let mut environment = Vec::new();
        for (key, value) in std::env::vars_os() {
            environment.extend(key.as_encoded_bytes());
            environment.push(b'=');
            environment.extend(value.as_encoded_bytes());
            environment.push(0);
        }
        Ok(hash == self.executable_hash
            && bytes == self.executable_bytes
            && environment_hash(environment.as_slice())? == self.initial_environment_hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifier_identity_binds_the_actual_running_rust_image() {
        let first = VerifierIdentity::capture().unwrap();
        assert_eq!(first.schema_version, 1);
        assert!(first.executable_bytes > 0);
        assert_eq!(first.executable_hash.len(), 64);
        assert_eq!(VerifierIdentity::capture().unwrap(), first);
    }
    #[test]
    fn identity_hashes_are_exact_and_bounded() {
        assert_eq!(
            hash_bounded(&b"abc"[..], 3).unwrap(),
            (
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".into(),
                3
            )
        );
        assert!(hash_bounded(&b"abc"[..], 2).is_err());
        assert_eq!(hash_bounded(&b""[..], 0).unwrap().1, 0);
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("identity read failed"))
            }
        }
        assert_eq!(
            hash_bounded(Broken, 1).unwrap_err().kind,
            NativeFailureKind::Io
        );
    }
    #[test]
    fn environment_identity_preserves_values_and_ignores_entry_order() {
        assert_eq!(
            environment_hash(&b"A=one\0B=two\0"[..]).unwrap(),
            environment_hash(&b"B=two\0A=one\0"[..]).unwrap()
        );
        assert_ne!(
            environment_hash(&b"A=one\0"[..]).unwrap(),
            environment_hash(&b"A=two\0"[..]).unwrap()
        );
        assert!(environment_hash(vec![b'x'; 1_048_577].as_slice()).is_err());
    }
}
