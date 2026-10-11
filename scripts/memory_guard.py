#!/usr/bin/env python3
"""Run a command in one Linux cgroup capped at 2/3 of available RAM."""
import os
from pathlib import Path
import subprocess
import sys


def memory_budget(text):
    for line in text.splitlines():
        fields = line.split()
        if fields and fields[0] == 'MemAvailable:':
            if len(fields) != 3 or fields[2] != 'kB':
                break
            available = int(fields[1]) * 1024
            if available > 0:
                return available * 2 // 3
            break
    raise ValueError('Cannot determine positive MemAvailable; refusing unbounded execution')


def guarded_run(command):
    if not command:
        raise ValueError('A command is required')
    budget = memory_budget(Path('/proc/meminfo').read_text())
    print(f'Memory guard: {budget} bytes total for command and descendants; swap disabled', flush=True)
    environment = {**os.environ, 'DCTX0_MEMORY_GUARDED': '1',
                   'DCTX0_MEMORY_BUDGET_BYTES': str(budget),
                   'CARGO_BUILD_JOBS': os.environ.get('CARGO_BUILD_JOBS', '2'),
                   'RUST_TEST_THREADS': os.environ.get('RUST_TEST_THREADS', '2')}
    # No fallback: an unavailable user manager must never silently remove the cap.
    return subprocess.run(['systemd-run', '--user', '--scope', '--quiet',
                           '--property', f'MemoryMax={budget}',
                           '--property', 'MemorySwapMax=0',
                           '--', *command],
                          env=environment).returncode


def guard_script():
    if os.environ.get('DCTX0_MEMORY_GUARDED') != '1':
        sys.exit(guarded_run([sys.executable, *sys.argv]))


if __name__ == '__main__':
    try:
        sys.exit(guarded_run(sys.argv[1:]))
    except (OSError, ValueError) as error:
        print(error, file=sys.stderr)
        sys.exit(1)
