#!/usr/bin/env python3
"""Cap Linux QA process-tree RSS at one third of startup available RAM."""
import ctypes
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys
import time


MIB = 1024 * 1024


def memory_budget(text):
    return available_memory(text) // 3


def available_memory(text):
    for line in text.splitlines():
        fields = line.split()
        if fields and fields[0] == 'MemAvailable:':
            if len(fields) != 3 or fields[2] != 'kB':
                break
            available = int(fields[1]) * 1024
            if available > 0:
                return available
            break
    raise ValueError('Cannot determine positive MemAvailable; refusing unbounded execution')


def processes():
    result = {}
    for directory in Path('/proc').iterdir():
        if not directory.name.isdecimal():
            continue
        try:
            fields = (directory / 'stat').read_text().rpartition(')')[2].split()
            rss = int((directory / 'statm').read_text().split()[1]) * os.sysconf('SC_PAGE_SIZE')
            result[int(directory.name)] = (int(fields[1]), int(fields[2]), fields[19], rss)
        except (OSError, ValueError, IndexError):
            continue
    return result


def sample(root_pid, tracked):
    current = processes()
    tracked = {pid: birth for pid, birth in tracked.items()
               if pid in current and current[pid][2] == birth}
    changed = True
    while changed:
        changed = False
        for pid, (parent, group, birth, _) in current.items():
            if pid not in tracked and (pid == root_pid or parent in tracked
                                      or group == root_pid or parent == os.getpid()):
                tracked[pid] = birth
                changed = True
    return tracked, sum(current[pid][3] for pid in tracked)


def subreaper():
    libc = ctypes.CDLL(None, use_errno=True)
    if libc.prctl(36, 1, 0, 0, 0) != 0:  # PR_SET_CHILD_SUBREAPER
        raise OSError(ctypes.get_errno(), 'Cannot own detached QA descendants')


def terminate(child, tracked):
    try:
        os.killpg(child.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    # Killing parents can expose adopted detached children. Resample until all
    # owned processes are gone; start times guard against PID reuse.
    for _ in range(100):
        tracked, _ = sample(child.pid, tracked)
        current = processes()
        for pid, birth in tracked.items():
            try:
                if current.get(pid, (None, None, None, None))[2] == birth:
                    os.kill(pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
        try:
            child.wait(timeout=0.1)
        except subprocess.TimeoutExpired:
            continue
        while True:
            try:
                pid, _ = os.waitpid(-1, os.WNOHANG)
            except ChildProcessError:
                break
            if pid == 0:
                break
        tracked, _ = sample(child.pid, tracked)
        if not tracked:
            return
        time.sleep(0.01)
    raise RuntimeError('QA descendants did not finish cleanup')


def monitored_run(command, budget, environment):
    subreaper()
    reserve = min(budget, 1024 * MIB)
    stopped = False

    def stop(_signum, _frame):
        nonlocal stopped
        stopped = True

    previous = {sig: signal.signal(sig, stop) for sig in (signal.SIGTERM, signal.SIGINT)}
    child = None
    tracked = {}
    peak = 0
    started = time.monotonic()
    try:
        child = subprocess.Popen(command, env=environment, start_new_session=True)
        while True:
            tracked, usage = sample(child.pid, tracked)
            peak = max(peak, usage)
            if stopped or usage > budget or available_memory(Path('/proc/meminfo').read_text()) < reserve:
                reason = 'signal' if stopped else 'RSS budget / system reserve'
                print(f'Memory guard: ABORT ({reason}); process-tree RSS {usage} bytes', flush=True)
                return 130 if stopped else 137
            code = child.poll()
            if code is not None:
                return code if code >= 0 else 128 - code
            if child.pid not in tracked:
                raise RuntimeError('Cannot observe the live QA command; refusing an unbounded run')
            time.sleep(0.05)
    finally:
        try:
            if child is not None:
                terminate(child, tracked)
        finally:
            for sig, handler in previous.items():
                signal.signal(sig, handler)
            elapsed = (time.monotonic() - started) * 1000
            print(f'Memory guard: elapsed_ms={elapsed:.3f}; peak_process_tree_rss_bytes={peak}; '
                  'method=sampled Linux process-tree RSS sum, 50ms, shared pages counted per process', flush=True)


def guarded_run(command):
    if not command:
        raise ValueError('A command is required')
    if os.environ.get('DCTX0_MEMORY_GUARD') == '0' or sys.platform != 'linux':
        print('Memory guard explicitly disabled or unsupported platform; running without a collective RAM cap', flush=True)
        return subprocess.run(command).returncode
    budget = memory_budget(Path('/proc/meminfo').read_text())
    print(f'Memory guard: {budget} bytes total for command and descendants '
          '(1/3 startup MemAvailable); independent RSS watchdog enabled', flush=True)
    environment = {**os.environ, 'DCTX0_MEMORY_GUARDED': '1',
                   'DCTX0_MEMORY_BUDGET_BYTES': str(budget),
                   'CARGO_BUILD_JOBS': os.environ.get('CARGO_BUILD_JOBS', '1'),
                   'RUST_TEST_THREADS': os.environ.get('RUST_TEST_THREADS', '1')}
    # A reachable systemd manager alone is not proof of memory enforcement (e.g.
    # Crostini). Use a real controller when exposed, and always keep the watchdog.
    if systemd_available():
        command = ['systemd-run', '--user', '--scope', '--quiet',
                   '--property', f'MemoryMax={budget}', '--property', 'MemorySwapMax=0',
                   '--', *command]
    return monitored_run(command, budget, environment)


def systemd_available():
    if sys.platform != 'linux' or not shutil.which('systemd-run') or not shutil.which('systemctl'):
        return False
    try:
        controllers = Path('/sys/fs/cgroup/cgroup.controllers').read_text().split()
        if 'memory' not in controllers:
            return False
        return subprocess.run(['systemctl', '--user', 'show-environment'],
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                              timeout=5).returncode == 0
    except (OSError, subprocess.TimeoutExpired):
        return False


def guard_script():
    if os.environ.get('DCTX0_MEMORY_GUARDED') != '1':
        sys.exit(guarded_run([sys.executable, *sys.argv]))


if __name__ == '__main__':
    try:
        sys.exit(guarded_run(sys.argv[1:]))
    except (OSError, ValueError, RuntimeError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
