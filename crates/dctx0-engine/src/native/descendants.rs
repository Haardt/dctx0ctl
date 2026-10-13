//! Track owned descendants that deliberately create independent process groups.
use super::*;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
#[derive(Default)]
pub(super) struct Descendants {
    identities: Arc<Mutex<BTreeMap<u32, String>>>,
    monitor: Option<JoinHandle<()>>,
    owner: String,
}
#[cfg(target_os = "linux")]
fn identity(pid: u32) -> Option<String> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // comm may contain spaces/parentheses; starttime is field 22.
    stat.rsplit_once(") ")?
        .1
        .split_whitespace()
        .nth(19)
        .map(str::to_owned)
}
#[cfg(target_os = "linux")]
fn scan(pid: u32, found: &mut BTreeMap<u32, String>, depth: usize) {
    if depth > 64 || found.len() >= 10_000 {
        return;
    }
    let Ok(children) = std::fs::read_to_string(format!("/proc/{pid}/task/{pid}/children")) else {
        return;
    };
    for child in children
        .split_whitespace()
        .filter_map(|text| text.parse::<u32>().ok())
    {
        if let Some(stamp) = identity(child) {
            found.insert(child, stamp);
            scan(child, found, depth + 1);
        }
    }
}
impl Descendants {
    pub fn start(pid: u32, owner: String) -> Self {
        let identities = Arc::new(Mutex::new(BTreeMap::new()));
        #[cfg(target_os = "linux")]
        let monitor = {
            let identities = identities.clone();
            Some(tokio::spawn(async move {
                loop {
                    {
                        let mut known = identities.lock().expect("descendant guard");
                        known.retain(|&child, stamp| identity(child).as_ref() == Some(stamp));
                        scan(pid, &mut known, 0);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }))
        };
        #[cfg(not(target_os = "linux"))]
        let monitor = {
            let _ = pid;
            None
        };
        Self {
            identities,
            monitor,
            owner,
        }
    }
    pub fn kill(&mut self, leader: u32) -> Result<(), NativeFailure> {
        #[cfg(target_os = "linux")]
        {
            let mut identities = self.identities.lock().expect("descendant guard");
            scan(leader, &mut identities, 0);
            // Keep the fallback off the periodic path: it is needed only when a
            // short-lived parent orphaned a child between monitor samples.
            use std::io::Read;
            for entry in std::fs::read_dir("/proc").map_err(NativeFailure::io)? {
                let entry = entry.map_err(NativeFailure::io)?;
                let Some(pid) = entry
                    .file_name()
                    .to_str()
                    .and_then(|name| name.parse::<u32>().ok())
                else {
                    continue;
                };
                let Ok(file) = std::fs::File::open(entry.path().join("environ")) else {
                    continue;
                };
                let mut environment = Vec::new();
                if file.take(1_048_577).read_to_end(&mut environment).is_err()
                    || environment.len() > 1_048_576
                {
                    continue;
                }
                if pid != leader
                    && environment
                        .split(|byte| *byte == 0)
                        .filter_map(|line| line.strip_prefix(b"DCTX0_NATIVE_OWNERS="))
                        .any(|chain| {
                            chain
                                .split(|byte| *byte == b':')
                                .any(|token| token == self.owner.as_bytes())
                        })
                {
                    if let Some(stamp) = identity(pid) {
                        identities.insert(pid, stamp);
                    }
                }
            }
            for (&pid, stamp) in identities.iter() {
                if identity(pid).as_ref() == Some(stamp) {
                    signal_result(rustix::process::kill_process(
                        rustix::process::Pid::from_raw(pid as i32).expect("OS pid"),
                        rustix::process::Signal::KILL,
                    ))?;
                }
            }
        }
        #[cfg(not(target_os = "linux"))]
        let _ = leader;
        if let Some(monitor) = self.monitor.take() {
            monitor.abort();
        }
        Ok(())
    }
    pub async fn reap(&self, timeout: Duration) -> Result<(), NativeFailure> {
        #[cfg(target_os = "linux")]
        {
            let deadline = Instant::now() + timeout;
            loop {
                if !self.reap_pending()? {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(NativeFailure::new(
                        NativeFailureKind::Cleanup,
                        "Nested descendants could not be reaped",
                    ));
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = timeout;
            Ok(())
        }
    }
    /// A live identity remains pending even before Linux reparents it to us.
    #[cfg(target_os = "linux")]
    pub(super) fn reap_pending(&self) -> Result<bool, NativeFailure> {
        let mut pending = false;
        for (&pid, stamp) in self.identities.lock().expect("descendant guard").iter() {
            if identity(pid).as_ref() != Some(stamp) {
                continue;
            }
            match rustix::process::waitpid(
                Some(rustix::process::Pid::from_raw(pid as i32).expect("OS pid")),
                rustix::process::WaitOptions::NOHANG,
            ) {
                Ok(Some(_)) => {}
                Err(rustix::io::Errno::CHILD) => pending |= identity(pid).as_ref() == Some(stamp),
                Ok(None) => pending = true,
                Err(error) => {
                    return Err(NativeFailure::new(
                        NativeFailureKind::Cleanup,
                        error.to_string(),
                    ));
                }
            }
        }
        Ok(pending)
    }
}
impl Drop for Descendants {
    fn drop(&mut self) {
        if let Some(monitor) = self.monitor.take() {
            monitor.abort();
        }
        #[cfg(target_os = "linux")]
        for (&pid, stamp) in self.identities.lock().expect("descendant guard").iter() {
            if identity(pid).as_ref() == Some(stamp) {
                let _ = rustix::process::kill_process(
                    rustix::process::Pid::from_raw(pid as i32).expect("OS pid"),
                    rustix::process::Signal::KILL,
                );
            }
        }
        #[cfg(target_os = "linux")]
        for (&pid, stamp) in self.identities.lock().expect("descendant guard").iter() {
            if identity(pid).as_ref() != Some(stamp) {
                continue;
            }
            let _ = rustix::process::waitpid(
                Some(rustix::process::Pid::from_raw(pid as i32).expect("OS pid")),
                rustix::process::WaitOptions::NOHANG,
            );
        }
    }
}
