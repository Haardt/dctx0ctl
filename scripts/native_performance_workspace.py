import contextlib
import ctypes
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import tempfile
import time
import memory_guard


def atomic_json(path, data):
    """Publish the latest receipt without truncating its previous valid version."""
    with tempfile.NamedTemporaryFile(mode='w', dir=path.parent, delete=False) as handle:
        json.dump(data, handle, indent=2)
        handle.write('\n')
        handle.flush()
        os.fsync(handle.fileno())
        temporary = handle.name
    os.replace(temporary, path)


def owned_processes(root, known, owner, scan_environment=False):
    current = memory_guard.processes()
    known = {pid: birth for pid, birth in known.items() if pid in current and current[pid][2] == birth}
    changed = True
    while changed:
        changed = False
        for pid, (parent, group, birth, _) in current.items():
            if pid not in known and (parent in known or (root in known and group == root)):
                known[pid] = birth
                changed = True
    if scan_environment:
        prefix = b'DCTX0_NATIVE_OWNERS='
        for pid, (_, _, birth, _) in current.items():
            if pid in known or pid == os.getpid():
                continue
            try:
                with open(f'/proc/{pid}/environ', 'rb') as environment:
                    entries = environment.read(1_048_577).split(b'\0')
                    if any(owner.encode() in entry[len(prefix):].split(b':') for entry in entries if entry.startswith(prefix)):
                        known[pid] = birth
            except OSError:
                continue
    return known


def cleanup_owned(root, known, owner):
    deadline = time.monotonic() + 5
    while True:
        known = owned_processes(root, known, owner, scan_environment=True)
        current = memory_guard.processes()
        for pid, birth in known.items():
            if current.get(pid, (None, None, None, None))[2] != birth:
                continue
            try:
                os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                continue
        current = memory_guard.processes()
        for pid, birth in known.items():
            if current.get(pid, (None, None, None, None))[2] != birth:
                continue
            try:
                os.waitpid(pid, os.WNOHANG)
            except ChildProcessError:
                pass
        known = owned_processes(root, known, owner, scan_environment=True)
        if not known:
            return
        if time.monotonic() >= deadline:
            raise RuntimeError('Native performance descendants did not finish cleanup')
        time.sleep(0.01)


def measure_json(command, cwd):
    """Measure the native CLI and clean only its identified Linux descendants."""
    started = time.perf_counter()
    libc = ctypes.CDLL(None, use_errno=True)
    previous = ctypes.c_int()
    if libc.prctl(37, ctypes.byref(previous), 0, 0, 0) != 0:
        raise OSError(ctypes.get_errno(), 'Cannot inspect native child subreaper')
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        owner = secrets.token_hex(16)
        def terminate(_signum, _frame):
            raise KeyboardInterrupt
        previous_term = signal.signal(signal.SIGTERM, terminate)
        process = None
        known = {}
        try:
            memory_guard.subreaper()
            owners = f'{os.environ.get("DCTX0_NATIVE_OWNERS", "")}:{owner}'
            process = subprocess.Popen(list(map(str, command)), cwd=cwd, env={**os.environ, 'DCTX0_MEMORY_GUARD': '0', 'DCTX0_NATIVE_OWNERS': owners}, stdout=stdout, stderr=stderr, start_new_session=True)
            current = memory_guard.processes()
            if process.pid in current:
                known[process.pid] = current[process.pid][2]
            while True:
                known = owned_processes(process.pid, known, owner)
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    process.returncode = os.waitstatus_to_exitcode(status)
                    break
                time.sleep(0.01)
        finally:
            try:
                if process is not None:
                    cleanup_owned(process.pid, known, owner)
                    if process.returncode is None:
                        process.returncode = -signal.SIGKILL
            finally:
                signal.signal(signal.SIGTERM, previous_term)
                if not previous.value and libc.prctl(36, 0, 0, 0, 0) != 0:
                    raise OSError(ctypes.get_errno(), 'Cannot restore native child subreaper')
        stdout.seek(0)
        stderr.seek(0)
        text, errors = stdout.read().decode(), stderr.read().decode()
    if process.returncode not in (0, 1):
        raise RuntimeError(errors)
    return json.loads(text), {'elapsed_ms': (time.perf_counter() - started) * 1000, 'peak_rss_bytes': int(usage.ru_maxrss) * 1024, 'exit_code': process.returncode}


@contextlib.contextmanager
def native_workspace(preserve):
    """Keep native temporary files on a short private on-disk path."""
    configured = os.environ.get('DCTX0_VALIDATION_TEMP_ROOT')
    base = Path(configured) if configured is not None else Path(os.environ.get('XDG_CACHE_HOME', Path.home() / '.cache')) / 'dctx0ctl'
    if not base.is_absolute():
        raise ValueError('DCTX0_VALIDATION_TEMP_ROOT must be absolute')
    base = base.resolve()
    if len(os.fsencode(base / ('v-' + 'x' * 8) / 'dctx0-ipc-ffffffff-ffffffffffffffff' / 's')) > 100:
        raise ValueError('Native temp root is too long for sockets')
    base.mkdir(mode=0o700, parents=True, exist_ok=True)
    kind = filesystem_type(base)
    if kind in ('tmpfs', 'ramfs', 'unavailable'):
        raise ValueError(f'Native temp root must be on a verifiable on-disk filesystem, got {kind}')
    context = contextlib.nullcontext(tempfile.mkdtemp(prefix='v-', dir=base)) if preserve else tempfile.TemporaryDirectory(prefix='v-', dir=base)
    with context as temporary:
        previous_env = os.environ.get('TMPDIR')
        previous_tempdir = tempfile.tempdir
        os.environ['TMPDIR'] = temporary
        tempfile.tempdir = temporary
        try:
            yield Path(temporary)
        finally:
            tempfile.tempdir = previous_tempdir
            if previous_env is None:
                os.environ.pop('TMPDIR', None)
            else:
                os.environ['TMPDIR'] = previous_env


def filesystem_type(path):
    try:
        result = subprocess.run(['stat', '-f', '-c', '%T', str(path)], capture_output=True, text=True, check=False)
    except OSError:
        return 'unavailable'
    return result.stdout.strip() if result.returncode == 0 else 'unavailable'
