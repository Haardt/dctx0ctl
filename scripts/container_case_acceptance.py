#!/usr/bin/env python3
"""Reproduce published-release installation and isolated CaseCli acceptance.
Requires Docker and the locally available golang:1.25.5 image. No host credentials
or project directories are mounted. Evidence includes child speed/RAM samples.
"""
import argparse
import json
from pathlib import Path
import subprocess
import tempfile
import hashlib
import os
from datetime import datetime, timezone
from release_support import locked
import time

ROOT = Path(__file__).resolve().parent.parent
FIXTURES = ROOT / 'scripts/container_case'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--name', default='dctx0-case-' + str(int(time.time())))
    parser.add_argument('--image', default='golang:1.25.5')
    parser.add_argument('--keep-running', action='store_true')
    args = parser.parse_args()
    output = ROOT / 'target/container-case' / args.name
    output.mkdir(parents=True, exist_ok=True)
    def command(*parts):
        subprocess.run(list(parts), check=True)
    command('docker', 'run', '-d', '--name', args.name, '--memory=3g', '--cpus=2', args.image, 'sleep', 'infinity')
    completed = False
    try:
        command('docker', 'exec', args.name, 'bash', '-c', 'mkdir -p /work/evidence; curl -fsSL https://github.com/Haardt/dctx0ctl/releases/latest/download/install.sh -o /work/install.sh; bash /work/install.sh')
        with tempfile.TemporaryDirectory(prefix='dctx0-container-') as staged:
            for name in ('setup', 'implement', 'accept', 'claims'):
                body = (FIXTURES / (name + '.py')).read_text()
                path = Path(staged) / (name + '.py')
                path.write_text(body)
                command('docker', 'cp', str(path), args.name + ':/work/' + path.name)
            test = Path(staged) / 'textcase_test.go'
            test.write_text((FIXTURES / 'textcase_test.go').read_text())
            for name in ('setup', 'implement', 'accept', 'claims'):
                if name == 'accept':
                    command('docker', 'cp', str(test), args.name + ':/work/casecli/textcase/textcase_test.go')
                command('docker', 'exec', '-e', 'PATH=/root/.local/bin:/usr/local/go/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin', '-e', 'GOMAXPROCS=2', args.name, 'python3', '-u', '/work/' + name + '.py')
        completed = True
    finally:
        subprocess.run(['docker', 'cp', args.name + ':/work/evidence/.', str(output)])
        metadata = {
            'base_image': args.image,
            'image_id': subprocess.check_output(['docker', 'inspect', args.name, '--format', '{{.Image}}'], text=True).strip(),
            'go': subprocess.check_output(['docker', 'exec', args.name, '/usr/local/go/bin/go', 'version'], text=True).strip(),
            'binary_sha256': subprocess.check_output(['docker', 'exec', args.name, 'sha256sum', '/root/.local/bin/dctx0'], text=True).split()[0],
            'revision': subprocess.check_output(['git', '-C', str(ROOT), 'rev-parse', 'HEAD'], text=True).strip(),
            'dirty_worktree': bool(subprocess.check_output(['git', '-C', str(ROOT), 'status', '--porcelain'], text=True).strip()),
            'fixture_hash': hashlib.sha256(b''.join(path.read_bytes() for path in sorted(FIXTURES.glob('*')) if path.is_file()) + Path(__file__).read_bytes()).hexdigest(),
            'container_memory_limit_bytes': 3 * 1024 ** 3,
            'container_cpus': 2,
            'cache': 'fresh container Go cache; subsequent verification uses warm cache; real network installer',
        }
        report = {'timestamp': datetime.now(timezone.utc).isoformat(), 'status': 'complete' if completed else 'incomplete',
                  'metadata': metadata, 'baseline_comparison': 'unassessed: isolated release consumer workload; core baseline unchanged'}
        summary = output / 'summary.json'
        if summary.exists():
            report['acceptance'] = json.loads(summary.read_text())
        report['measurement_sets'] = {path.name: json.loads(path.read_text()) for path in output.glob('*measurements.json')}
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
        history = ROOT / 'benchmarks/history/container-case'
        history.mkdir(parents=True, exist_ok=True)
        with locked(history.parent / '.container-case.lock'):
            destination = history / (datetime.now(timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ') + '.json')
            with tempfile.NamedTemporaryFile(mode='w', dir=history, delete=False) as staged_report:
                staged_report.write(json.dumps(report, indent=2) + '\n')
                staged_report.flush()
                os.fsync(staged_report.fileno())
            os.replace(staged_report.name, destination)
            for old in sorted(history.glob('*.json'))[:-20]:
                old.unlink()
        if not args.keep_running:
            subprocess.run(['docker', 'stop', args.name], check=True)
    print(json.dumps({'container': args.name, 'evidence': str(output)}, indent=2))

if __name__ == '__main__':
    main()
