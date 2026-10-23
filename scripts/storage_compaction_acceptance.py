#!/usr/bin/env python3
"""Compare a compacted copy with a consistent old-schema reference, including speed/RSS."""
import argparse
import datetime
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import statistics
import subprocess
import tempfile
import time

import validate
from query_acceptance import TIMER_SOURCE

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--spec', type=Path, required=True)
    parser.add_argument('--copy', type=Path, required=True)
    parser.add_argument('--old-binary', type=Path, required=True)
    parser.add_argument('--new-binary', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--samples', type=int, default=5)
    args = parser.parse_args()
    if args.samples < 1:
        parser.error('positive sample count required')
    spec = args.spec.resolve()
    source = spec / '.dctx0ctl/state.sqlite3'
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    folder = ROOT / 'target/storage-comparison' / stamp
    folder.mkdir(parents=True)
    old = args.old_binary.resolve()
    new = args.new_binary.resolve()
    compact = args.copy.resolve()
    reference = folder / 'reference.sqlite3'
    report = {'schema_version': 1, 'status': 'incomplete', 'timestamp': stamp,
              'source': str(source), 'copy': str(compact), 'checks': [], 'measurements': {},
              'source_hash_before': validate.digest(validate.validation_sources()),
              'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': validate.capture(['git', 'status', '--porcelain']),
              'platform': validate.capture(['uname', '-a']),
              'hardware': {'cpu_count': os.cpu_count(), 'cpu': validate.capture(['lscpu'])},
              'toolchain': validate.capture(['rustc', '--version']),
              'settings': {'samples': args.samples, 'warmup': 1, 'iterations': 1,
                           'cache': 'warm OS cache; separate CLI process per sample',
                           'profile': 'release, uninstrumented', 'speed_tolerance_percent': 20,
                           'ram_tolerance_percent': 10},
              'memory_scope': 'native wait4 child peak RSS, excludes Python driver and reference preparation',
              'baseline': 'paired local comparison only; accepted baseline unchanged',
              'binary_hashes': {'old': hashlib.sha256(old.read_bytes()).hexdigest(),
                                'new': hashlib.sha256(new.read_bytes()).hexdigest()}}
    timer = folder / 'timer'
    sequence = 0

    def cli(label, binary, database, *command):
        nonlocal sequence
        sequence += 1
        rss = folder / f'{sequence:04}.rss'
        output = folder / f'{sequence:04}-{label}.log'
        cmd = [str(timer), str(rss), str(binary), '--db', str(database), *map(str, command), '--json']
        with output.open('wb') as stream:
            result = subprocess.run(cmd, stdout=stream, stderr=subprocess.STDOUT, timeout=900, cwd=ROOT)
        measured = json.loads(rss.read_text())
        measured.update({'command': cmd, 'exit_code': result.returncode, 'log': str(output.relative_to(ROOT))})
        report['checks'].append(measured)
        if result.returncode or measured['peak_rss_bytes'] <= 0:
            raise RuntimeError(f'{label} failed: {output}')
        contents = output.read_text()
        # Export's contract is the atomically written file; older supported
        # binaries deliberately emit no JSON receipt on stdout.
        body = None if not contents.strip() and command[:2] == ('verify', 'export') else json.loads(contents)
        return measured, body

    try:
        (folder / 'timer.c').write_text(TIMER_SOURCE)
        subprocess.run([shutil.which('cc'), '-O2', '-Wall', '-Wextra', '-Werror',
                        str(folder / 'timer.c'), '-o', str(timer)], check=True)
        started = time.perf_counter()
        with sqlite3.connect(f'file:{source}?mode=ro', uri=True) as original:
            with sqlite3.connect(reference) as target:
                original.backup(target)
        report['reference_backup_ms'] = (time.perf_counter() - started) * 1000
        _, verification = cli('verify-copy', new, reference, 'storage', 'verify-copy', spec, '--copy', compact)
        report['verification'] = verification
        cli('prepare-reference', old, reference, 'query', 'prepare', spec)
        with sqlite3.connect(reference) as db:
            plan, event, snapshot = db.execute('SELECT id,created_seq,snapshot_id FROM task_plans ORDER BY id LIMIT 1').fetchone()
        cli('prepare-history', old, reference, 'query', 'prepare', spec, '--snapshot', snapshot)
        sql_workloads = {
            'task-counts': ('SELECT work,check_status,COUNT(*) FROM dctx0_tasks GROUP BY work,check_status ORDER BY work,check_status', []),
            'ready-tasks': ("SELECT task_id,ready,delivered FROM dctx0_tasks WHERE work='Open' ORDER BY wave,task_id LIMIT 100", []),
            'source-join': ('SELECT t.task_id,s.subject,r.file,r.start_line FROM dctx0_tasks t JOIN dctx0_task_subjects s USING(task_id) JOIN dctx0_source_refs r ON r.subject=s.subject ORDER BY t.task_id,s.subject,r.file,r.start_line LIMIT 100', []),
            'case-counts': ('SELECT subject,outcome,COUNT(*) FROM dctx0_cases GROUP BY subject,outcome ORDER BY subject,outcome', []),
            'spec-search': ("SELECT subject,text FROM dctx0_facts WHERE text LIKE :text ORDER BY subject LIMIT 10", ['--param', 'text="%Company%"']),
            'historical-tasks': ('SELECT work,check_status,COUNT(*) FROM dctx0_tasks GROUP BY work,check_status ORDER BY work,check_status', ['--plan', str(plan), '--as-of', str(event)]),
        }
        # Case metadata now outlives bodies; restrict the paired query to runs with
        # details retained in both files. No filtered result is presented as overall PASS.
        with sqlite3.connect(compact) as db:
            run = db.execute('SELECT id FROM verification_runs WHERE pruned_at IS NULL ORDER BY rowid DESC LIMIT 1').fetchone()
            case = db.execute('SELECT case_id FROM verification_case_index WHERE run_id=? ORDER BY ordinal LIMIT 1', run).fetchone() if run else None
        if run:
            sql_workloads['case-counts'] = ('SELECT subject,outcome,COUNT(*) FROM dctx0_cases WHERE run_id=:run GROUP BY subject,outcome ORDER BY subject,outcome', ['--param', 'run=' + json.dumps(run[0])])
        for name, (sql, extras) in sql_workloads.items():
            results = {}
            for label, binary, database in [('old', old, reference), ('new', new, compact)]:
                cli(name + '-' + label + '-warmup', binary, database, 'query', 'sql', spec, '--sql', sql, '--timeout-seconds', '60', *extras)
                samples = []
                for _ in range(args.samples):
                    sample, body = cli(name + '-' + label, binary, database, 'query', 'sql', spec, '--sql', sql, '--timeout-seconds', '60', *extras)
                    samples.append(sample)
                results[label] = {'samples': samples, 'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                                  'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples),
                                  'result': {k: body[k] for k in ('pins', 'columns', 'rows', 'complete')}}
            if results['old']['result'] != results['new']['result']:
                raise RuntimeError(f'{name}: query results differ')
            del results['old']['result'], results['new']['result']
            for metric in ('elapsed_ms', 'peak_rss_bytes'):
                before, after = results['old'][metric], results['new'][metric]
                results[metric + '_delta'] = after - before
                results[metric + '_delta_percent'] = (after / before - 1) * 100
            report['measurements'][name] = results
        if run and case:
            detail = {}
            for label, binary, database in [('old', old, reference), ('new', new, compact)]:
                sample, body = cli('single-case-' + label, binary, database, 'verify', 'show', spec, run[0], '--case', case[0], '--max-output-bytes', '67108864')
                detail[label] = body.get('detail')
                report['measurements']['single-case-' + label] = sample
            if detail['old'] != detail['new']:
                raise RuntimeError('Single-case bodies differ')
        if run:
            exports = {}
            for label, binary, database in [('old', old, reference), ('new', new, compact)]:
                destination = folder / (label + '-full.json')
                cli('export-' + label + '-warmup', binary, database, 'verify', 'export', spec, run[0], '--output', destination)
                samples = [cli('export-' + label, binary, database, 'verify', 'export', spec, run[0], '--output', destination)[0] for _ in range(args.samples)]
                exports[label] = {'samples': samples, 'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                                  'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples),
                                  'sha256': hashlib.file_digest(destination.open('rb'), 'sha256').hexdigest(),
                                  'bytes': destination.stat().st_size}
            if exports['old']['sha256'] != exports['new']['sha256']:
                raise RuntimeError('Full canonical exports differ')
            for metric in ('elapsed_ms', 'peak_rss_bytes'):
                before, after = exports['old'][metric], exports['new'][metric]
                exports[metric + '_delta'] = after - before
                exports[metric + '_delta_percent'] = (after / before - 1) * 100
            report['measurements']['full-export'] = exports
        report['file_bytes_before'] = source.stat().st_size
        report['file_bytes_after'] = compact.stat().st_size
        with compact.open('rb') as file:
            report['copy_sha256'] = hashlib.file_digest(file, 'sha256').hexdigest()
        report['source_hash_after'] = validate.digest(validate.validation_sources())
        regressions = [name for name, values in report['measurements'].items()
                       if 'elapsed_ms_delta_percent' in values and
                       (values['elapsed_ms_delta_percent'] > 20 or values['peak_rss_bytes_delta_percent'] > 10)]
        report['regressions'] = regressions
        report['status'] = ('failed-regression' if regressions else 'complete') if report['source_hash_before'] == report['source_hash_after'] else 'failed-source-drift'
    except Exception as error:
        report['error'] = str(error)
        print(error, flush=True)
    finally:
        history = ROOT / 'benchmarks/history/compaction'
        history.mkdir(parents=True, exist_ok=True)
        with (history / '.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            destination = history / f'{stamp}-{os.getpid()}.json'
            with tempfile.NamedTemporaryFile('w', dir=history, delete=False) as out:
                json.dump(report, out, indent=2)
                out.write('\n')
                out.flush()
                os.fsync(out.fileno())
            os.replace(out.name, destination)
            for previous in sorted(history.glob('*.json'))[:-20]:
                previous.unlink()
        print('Compaction evidence:', destination.relative_to(ROOT), flush=True)
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    raise SystemExit(main())
