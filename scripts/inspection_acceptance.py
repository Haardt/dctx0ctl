#!/usr/bin/env python3
"""Couple large bound-review acceptance with isolated release CLI speed and RAM."""
import datetime
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile
from release_support import locked, measured
import validate

ROOT = Path(__file__).resolve().parents[1]


def main():
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    folder = ROOT / 'target/inspection-acceptance' / stamp
    folder.mkdir(parents=True)
    metrics = folder / 'cli.jsonl'
    source = validate.digest(validate.validation_sources())
    environment = {**os.environ, 'DCTX0_TEST_PYTHON': sys.executable,
                   'DCTX0_INSPECTION_METRICS': str(metrics),
                   'DCTX0_INSPECTION_BINARY': str(ROOT / 'target/release/dctx0')}
    report = {'schema_version': 1, 'timestamp': stamp, 'status': 'incomplete',
              'source_hash_before': source, 'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': bool(validate.capture(['git', 'status', '--porcelain'])),
              'toolchain': validate.capture(['rustc', '--version']), 'platform': platform.platform(),
              'hardware': {'machine': platform.machine(), 'cpu_count': os.cpu_count()},
              'scope': 'Real Bun native corpus, 543 source modules, 19/47 exactly bound reviews; retained unreviewed UNKNOWN, rejected bindings and resource caps',
              'settings': {'profile': 'release CLI; debug fixture/test orchestration', 'samples': 1,
                           'seed': 42, 'cache': 'existing compiler caches; fresh isolated project',
                           'memory_scope': 'Each CLI invocation uses wait4 peak RSS including its reaped children; inspection invokes no compiler or native harness. Build/test costs separately retained.'},
              'baseline': 'unassessed; accepted baselines unchanged', 'checks': []}
    try:
        commands = [['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'],
                    ['cargo', 'test', '-p', 'dctx0-cli', '--test', 'verification_inspection', '--test', 'verification_inspection_large', '--all-features', '--locked']]
        for index, command in enumerate(commands):
            sample, output = measured(command, ROOT, timeout=1800, env=environment)
            (folder / f'check-{index}.log').write_text(output)
            report['checks'].append({'command': command, **sample})
            if sample['exit_code']:
                raise RuntimeError(f'Acceptance failed: {command}; see {folder}')
        report['cli_measurements'] = [json.loads(line) for line in metrics.read_text().splitlines()]
        report['source_hash_after'] = validate.digest(validate.validation_sources())
        if source != report['source_hash_after']:
            raise RuntimeError('Acceptance inputs changed; result does not certify the final sources')
        report['status'] = 'complete'
    except Exception as error:
        report['error'] = str(error)
    finally:
        history = ROOT / 'benchmarks/history/inspection'
        history.mkdir(parents=True, exist_ok=True)
        with locked(history / '.lock'):
            with tempfile.NamedTemporaryFile(mode='w', dir=history, delete=False) as staged:
                json.dump(report, staged, indent=2)
                staged.write('\n')
                staged.flush()
                os.fsync(staged.fileno())
            destination = history / f'{stamp}-{os.getpid()}.json'
            os.replace(staged.name, destination)
            for old in sorted(history.glob('*.json'))[:-20]:
                old.unlink()
        print(json.dumps({'status': report['status'], 'report': str(destination), 'error': report.get('error')}))
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    sys.exit(main())
