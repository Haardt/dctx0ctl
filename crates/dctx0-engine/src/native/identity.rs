//! Identity read by the managing process, independent of worker claims.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeRuntimeIdentity {
    pub schema_version: u32,
    #[serde(default = "live_method")]
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub launch: Option<NativeLaunchInputs>,
    pub executable_hash: String,
    pub executable_path: String,
    pub executable_bytes: u64,
    pub initial_environment_hash: String,
}

fn live_method() -> String {
    "procfs-live-image".into()
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NativeLaunchInputs {
    pub arguments: Vec<String>,
    pub directory: String,
    pub parent_environment_hash: String,
}

/// Resolve once and launch this path. Keep the final symlink name: rustup uses
/// argv[0] to select rustc/cargo, so canonicalizing that name changes the program.
pub(crate) fn resolve_program(
    program: &str,
    directory: &Path,
) -> Result<std::path::PathBuf, NativeFailure> {
    let path = Path::new(program);
    let roots = if path.components().count() > 1 || path.is_absolute() {
        vec![directory.to_path_buf()]
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .map(|root| {
                if root.is_absolute() {
                    root
                } else {
                    directory.join(root)
                }
            })
            .collect()
    };
    for root in roots {
        let candidate = root.join(path);
        #[cfg(windows)]
        let candidates = if candidate.extension().is_none() {
            vec![candidate.with_extension("exe"), candidate]
        } else {
            vec![candidate]
        };
        #[cfg(not(windows))]
        let candidates = vec![candidate];
        for candidate in candidates {
            if candidate.is_file() {
                return Ok(if candidate.is_absolute() {
                    candidate
                } else {
                    std::env::current_dir()
                        .map_err(NativeFailure::io)?
                        .join(candidate)
                });
            }
        }
    }
    Err(NativeFailure::new(
        NativeFailureKind::Unavailable,
        format!("Executable unavailable: {program}"),
    ))
}
fn parent_environment() -> Result<String, NativeFailure> {
    environment_entries(std::env::vars_os().collect())
}
// as_encoded_bytes is the platform-native, lossless OsStr encoding. Hash sorted
// entries with NUL delimiters; values are never persisted. Windows names compare
// case-insensitively when applying Command overrides.
fn environment_entries(
    entries: std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString>,
) -> Result<String, NativeFailure> {
    let mut bytes = Vec::new();
    for (key, value) in entries {
        bytes.extend(key.as_encoded_bytes());
        bytes.push(b'=');
        bytes.extend(value.as_encoded_bytes());
        bytes.push(0);
    }
    environment_hash(bytes.as_slice())
}
impl NativeRuntimeIdentity {
    pub(super) fn capture_launch(
        program: &Path,
        recipe: &ExecutionRecipe,
        directory: &Path,
        command: &mut Command,
    ) -> Result<Self, NativeFailure> {
        let mut environment = std::env::vars_os().collect::<std::collections::BTreeMap<_, _>>();
        for (key, value) in command.as_std().get_envs() {
            #[cfg(windows)]
            environment.retain(|name, _| {
                !name
                    .to_string_lossy()
                    .eq_ignore_ascii_case(&key.to_string_lossy())
            });
            if let Some(value) = value {
                environment.insert(key.to_owned(), value.to_owned());
            } else {
                environment.remove(key);
            }
        }
        let initial_environment_hash = environment_entries(environment.clone())?;
        // Freeze the exact effective environment used to compute the digest.
        command.env_clear().envs(environment);
        let (executable_hash, executable_bytes) = hash_bounded(
            std::fs::File::open(program).map_err(NativeFailure::io)?,
            268_435_456,
        )?;
        Ok(Self {
            schema_version: 2,
            method: "captured-launch".into(),
            launch: Some(NativeLaunchInputs {
                arguments: recipe.arguments.clone(),
                directory: std::fs::canonicalize(directory)
                    .map_err(NativeFailure::io)?
                    .to_string_lossy()
                    .into_owned(),
                parent_environment_hash: parent_environment()?,
            }),
            executable_hash,
            executable_bytes,
            executable_path: program.to_string_lossy().into_owned(),
            initial_environment_hash,
        })
    }
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
        #[cfg(target_os = "linux")]
        let path = std::path::PathBuf::from("/proc/self/exe");
        #[cfg(not(target_os = "linux"))]
        let path = std::env::current_exe().map_err(NativeFailure::io)?;
        let file = std::fs::File::open(path).map_err(NativeFailure::io)?;
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
        if !self.supervised {
            return Ok(self.launch_identity.clone());
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
            method: live_method(),
            launch: None,
            executable_hash,
            executable_path,
            executable_bytes,
            initial_environment_hash,
        })
    }
}

// Process ownership tokens are ephemeral supervisor metadata, not application
// environment inputs. All other names and values remain bound exactly.
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
        .filter(|entry| !entry.is_empty() && !entry.starts_with(b"DCTX0_NATIVE_OWNERS="))
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
        if !matches!(self.schema_version, 1 | 2) || self.executable_path.is_empty() {
            return Ok(false);
        }
        let file = std::fs::File::open(&self.executable_path).map_err(NativeFailure::io)?;
        let (hash, bytes) = hash_bounded(file, 268_435_456)?;
        if self.schema_version == 2 {
            let Some(launch) = &self.launch else {
                return Ok(false);
            };
            return Ok(self.method == "captured-launch"
                && Path::new(&launch.directory).is_absolute()
                && hash == self.executable_hash
                && bytes == self.executable_bytes
                && parent_environment()? == launch.parent_environment_hash);
        }
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

#[cfg(all(test, target_os = "linux"))]
mod managed_environment_tests {
    use super::*;
    #[tokio::test]
    async fn runtime_identity_ignores_only_supervisor_tokens_and_remains_current() {
        let recipe=ExecutionRecipe{program:"python3".into(),arguments:vec!["-c".into(),r#"import sys,json,time
request=json.loads(sys.stdin.readline());print(json.dumps({'protocol_version':1,'message':{'kind':'reply','id':request['id'],'reply':{'kind':'described','description':{'protocol_version':1,'subjects':[],'artifacts':[],'capabilities':['sessions'],'representations':{}}}}}),flush=True);time.sleep(100)
"#.into()]};
        let mut worker = NativeWorker::spawn(&recipe, Path::new("/tmp"), NativeLimits::default())
            .await
            .unwrap();
        worker
            .request(
                WorkerRequest::Describe,
                &NativeCancellation::default(),
                &mut |_invocation| unreachable!(),
            )
            .await
            .unwrap();
        let identity = worker.runtime_identity().unwrap();
        assert!(identity.still_current().unwrap());
        let mut invalid = identity.clone();
        invalid.schema_version = 99;
        assert!(!invalid.still_current().unwrap());
        assert_eq!(
            environment_hash(b"A=1\0DCTX0_NATIVE_OWNERS=one:two\0".as_slice()).unwrap(),
            environment_hash(b"A=1\0".as_slice()).unwrap()
        );
        assert_ne!(
            environment_hash(b"A=1\0".as_slice()).unwrap(),
            environment_hash(b"A=2\0".as_slice()).unwrap()
        );
        worker.abort().await.unwrap();
    }
}
