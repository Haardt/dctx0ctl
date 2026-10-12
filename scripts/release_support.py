"""Portable distribution locks, bounded child execution, and explicit OS memory metrics."""
from contextlib import contextmanager
import os
import platform
import signal
import subprocess
import tempfile
import time


@contextmanager
def locked(path):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open('a+b') as handle:
        if os.name == 'nt':
            import msvcrt
            if handle.tell() == 0:
                handle.write(b'0')
                handle.flush()
            while True:
                handle.seek(0)
                try:
                    msvcrt.locking(handle.fileno(), msvcrt.LK_NBLCK, 1)
                    break
                except OSError:
                    time.sleep(0.1)
            try:
                yield
            finally:
                handle.seek(0)
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
        else:
            import fcntl
            fcntl.flock(handle, fcntl.LOCK_EX)
            yield


def windows_peak(pid):
    import ctypes
    from ctypes import wintypes
    class Counters(ctypes.Structure):
        _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD)] + [
            (name, ctypes.c_size_t) for name in ('PeakWorkingSetSize', 'WorkingSetSize',
            'QuotaPeakPagedPoolUsage', 'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage',
            'QuotaNonPagedPoolUsage', 'PagefileUsage', 'PeakPagefileUsage')]
    kernel = ctypes.WinDLL('kernel32', use_last_error=True)
    api = ctypes.WinDLL('psapi', use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    api.GetProcessMemoryInfo.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    api.GetProcessMemoryInfo.restype = wintypes.BOOL
    handle = kernel.OpenProcess(0x0400 | 0x0010, False, pid)
    if not handle:
        return None
    try:
        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        if not api.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
            return None
        return counters.PeakWorkingSetSize or None
    finally:
        kernel.CloseHandle(handle)


def measured(command, cwd, timeout=300, env=None):
    started = time.perf_counter()
    usage = None
    rss = None
    with tempfile.TemporaryFile() as output:
        child = subprocess.Popen(command, cwd=cwd, stdout=output, stderr=output,
                                 env=env, start_new_session=os.name != 'nt',
                                 creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == 'nt' else 0)
        try:
            while True:
                if os.name == 'nt':
                    peak = windows_peak(child.pid)
                    rss = max(rss or 0, peak) if peak else rss
                    if child.poll() is not None:
                        break
                else:
                    pid, status, usage = os.wait4(child.pid, os.WNOHANG)
                    if pid:
                        child.returncode = os.waitstatus_to_exitcode(status)
                        rss = usage.ru_maxrss * (1024 if platform.system() == 'Linux' else 1)
                        break
                if time.perf_counter() - started > timeout:
                    raise TimeoutError(f'Timed out: {command[0]}')
                time.sleep(0.01)
        finally:
            if child.returncode is None:
                if os.name == 'nt':
                    subprocess.run(['taskkill.exe', '/PID', str(child.pid), '/T', '/F'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=10, check=False)
                    child.kill()
                    child.wait(timeout=10)
                else:
                    os.killpg(child.pid, signal.SIGKILL)
                    _, status, usage = os.wait4(child.pid, 0)
                    child.returncode = os.waitstatus_to_exitcode(status)
        output.seek(0)
        text = output.read().decode('utf-8', errors='replace')
    return {'elapsed_ms': (time.perf_counter() - started) * 1000,
            'peak_rss_bytes': rss, 'exit_code': child.returncode,
            'memory_status': 'measured' if rss is not None else 'unavailable: OS peak-memory query failed',
            'memory_scope': 'Windows child PeakWorkingSetSize (polled OS high-water mark); excludes descendants' if os.name == 'nt'
                            else 'wait4 child/reaped descendant peak RSS; may include inherited memory before exec; not collective RSS'}, text
