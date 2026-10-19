#!/usr/bin/env python3
"""Qualify the real Go race detector, then run uncached Cinema tests with measurements."""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import time

from release_support import measured

ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / 'examples/cinema-go'
REPORT = ROOT / 'target/go-cinema/race.json'
POSITIVE = '''package main
import "fmt"
func main() { fmt.Println("RACE_PROBE_OK") }
'''
NEGATIVE = '''package main
import "fmt"
var value int
func main() {
    done := make(chan bool)
    go func() { value = 1; done <- true }()
    value = 2
    <-done
    fmt.Println(value)
}
'''


class RaceUnavailable(Exception):
    pass


def source_hash(project):
    digest = hashlib.sha256()
    for path in sorted(project.rglob('*')):
        relative = path.relative_to(project)
        if any(part.startswith('.') or part in ('target', 'node_modules') for part in relative.parts):
            continue
        if path.is_file() and (path.suffix == '.go' or path.name in ('go.mod', 'go.sum')):
            digest.update(str(relative).encode() + b'\0' + path.read_bytes())
    return digest.hexdigest()


def save(report, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(mode='w', dir=destination.parent, delete=False) as staged:
        json.dump(report, staged, indent=2)
        staged.write('\n')
        staged.flush()
        os.fsync(staged.fileno())
    os.replace(staged.name, destination)


def unsupported_vma(output, host):
    return (host.get('GOOS') == 'linux' and host.get('GOARCH') == 'arm64'
            and 'FATAL: ThreadSanitizer: unsupported VMA range' in output
            and re.search(r'FATAL: Found (39|42) - Supported 48(?:\s|$)', output) is not None)


def run(project=PROJECT, destination=REPORT):
    started = time.time_ns()
    report = {'schema_version': 1, 'status': 'incomplete', 'started_at_unix_ns': str(started),
              'source_hash_before': source_hash(project), 'checks': [],
              'host': {'platform': platform.platform(), 'machine': platform.machine()},
              'memory_scope': 'Per-command wait4 peak child/reaped descendants; not aggregate simultaneous RSS. Outer Linux guard caps sampled process-tree RSS at one third startup MemAvailable.',
              'scope': 'Native race detector qualification and uncached Go Cinema package tests; no imported PASS or plain-test fallback'}
    save(report, destination)
    environment = {**os.environ, 'GOWORK': 'off', 'GOFLAGS': '',
                   'GORACE': 'exitcode=66 halt_on_error=1 log_path=stderr',
                   'GOMAXPROCS': os.environ.get('GOMAXPROCS', '2')}

    def check(label, command):
        metrics, output = measured(command, project, timeout=900, env=environment)
        report['checks'].append({'name': label, 'command': list(map(str, command)),
                                 'measurement': metrics, 'output': output})
        save(report, destination)
        return metrics['exit_code'], output

    code = 1
    try:
        go = shutil.which('go')
        if not go:
            raise RaceUnavailable('GO_TOOLCHAIN_UNAVAILABLE')
        rc, output = check('toolchain', [go, 'env', '-json', 'GOVERSION', 'GOROOT', 'GOOS', 'GOARCH', 'GOHOSTOS', 'GOHOSTARCH', 'CGO_ENABLED'])
        if rc:
            raise RuntimeError('Cannot inspect the selected Go toolchain')
        report['host'].update(json.loads(output))
        if report['host']['GOOS'] != report['host']['GOHOSTOS'] or report['host']['GOARCH'] != report['host']['GOHOSTARCH']:
            raise RaceUnavailable('NATIVE_RACE_HOST_REQUIRED')
        with tempfile.TemporaryDirectory(prefix='go-race-probe-', dir=destination.parent) as temporary:
            directory = Path(temporary)
            for label, source in [('positive', POSITIVE), ('negative', NEGATIVE)]:
                path = directory / (label + '.go')
                binary = directory / (label + ('.exe' if os.name == 'nt' else ''))
                path.write_text(source)
                rc, output = check(label + '-build', [go, 'build', '-race', '-p', '1', '-o', str(binary), str(path)])
                if rc:
                    if '-race requires cgo' in output or '-race is not supported' in output:
                        raise RaceUnavailable('RACE_TOOLCHAIN_UNAVAILABLE')
                    raise RuntimeError('Race probe compilation failed')
                rc, output = check(label + '-execution', [str(binary)])
                if label == 'positive':
                    if rc and unsupported_vma(output, report['host']):
                        report['explanation'] = 'ARM64 Go race requires 48-bit VMA; this kernel exposes 39/42 bits. Run on a supported host, such as the required Linux x64 release CI. No application race tests executed.'
                        raise RaceUnavailable('RACE_HOST_VMA_UNSUPPORTED')
                    if rc or output.strip() != 'RACE_PROBE_OK':
                        raise RuntimeError('Race runtime did not complete the clean probe')
                elif rc != 66 or 'WARNING: DATA RACE' not in output:
                    raise RuntimeError('Race runtime did not detect the deliberately racy probe')
        rc, _ = check('cinema-race', [go, 'test', '-race', '-count=1', '-mod=readonly', '-p', '1', './...'])
        if rc:
            raise RuntimeError('Cinema race tests failed; see actual test output')
        code = 0
        report['status'] = 'pass'
    except RaceUnavailable as error:
        code = 2
        report.update(status='unavailable', issue=str(error))
    except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.SubprocessError) as error:
        report.update(status='fail', issue='GO_RACE_FAILED', error=str(error))
    finally:
        report['source_hash_after'] = source_hash(project)
        if report['source_hash_before'] != report['source_hash_after']:
            report.update(status='fail', issue='RACE_SOURCE_CHANGED')
            code = 1
        report['elapsed_ms'] = (time.time_ns() - started) / 1_000_000
        save(report, destination)
        print(json.dumps({'status': report['status'], 'issue': report.get('issue'),
                          'report': str(destination), 'elapsed_ms': report['elapsed_ms']}), flush=True)
    return code


if __name__ == '__main__':
    from memory_guard import guard_script
    guard_script()
    sys.exit(run())
