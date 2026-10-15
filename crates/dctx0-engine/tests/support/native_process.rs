//! Python is a test fixture only; the coupled runner supplies its interpreter.
use dctx0_target_sdk::ExecutionRecipe;
use std::{path::Path, process::Command};
pub fn recipe(code: &str) -> ExecutionRecipe {
    let python = std::env::var("DCTX0_TEST_PYTHON").expect("Set DCTX0_TEST_PYTHON to the runner's absolute Python interpreter (scripts/validate.py and native_portability.py do this)");
    assert!(Path::new(&python).is_absolute());
    ExecutionRecipe {
        program: python,
        arguments: vec!["-u".into(), "-c".into(), code.into()],
    }
}
pub fn assert_exited(pid: u32) {
    let code = r#"import os,sys
pid=int(sys.argv[1])
if os.name=='nt':
 import ctypes
 from ctypes import wintypes
 api=ctypes.WinDLL('kernel32',use_last_error=True)
 api.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD]
 api.OpenProcess.restype=wintypes.HANDLE
 api.GetExitCodeProcess.argtypes=[wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)]
 api.GetExitCodeProcess.restype=wintypes.BOOL
 api.CloseHandle.argtypes=[wintypes.HANDLE]
 handle=api.OpenProcess(0x1000,False,pid)
 if not handle:
  if ctypes.get_last_error() in (87,1168):sys.exit(0)
  raise ctypes.WinError(ctypes.get_last_error())
 try:
  code=wintypes.DWORD()
  if not api.GetExitCodeProcess(handle,ctypes.byref(code)):raise ctypes.WinError(ctypes.get_last_error())
  sys.exit(1 if code.value==259 else 0)
 finally:api.CloseHandle(handle)
else:
 try:os.kill(pid,0)
 except ProcessLookupError:sys.exit(0)
 sys.exit(1)
"#;
    let fixture = recipe(code);
    let result = Command::new(fixture.program)
        .args(fixture.arguments)
        .arg(pid.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "Native child {pid} remains alive or cannot be inspected: {}",
        String::from_utf8_lossy(&result.stderr)
    );
}
pub fn peak_rss(pid: u32) -> Option<u64> {
    std::fs::read_to_string(format!("/proc/{pid}/status"))
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()?
                .checked_mul(1024)
        })
}
pub fn copy_tree(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).unwrap();
    for entry in std::fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}
