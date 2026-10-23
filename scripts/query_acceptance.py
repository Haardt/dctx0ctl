#!/usr/bin/env python3
"""Couple query/storage CLI acceptance with release speed/RSS and size measurements."""
import argparse
import datetime
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import sqlite3
import statistics
import sys
import tempfile
import time

import validate
from storage_acceptance import run

ROOT = Path(__file__).resolve().parents[1]


TIMER_SOURCE = r"""
#define _GNU_SOURCE
#include <errno.h>
#include <stdio.h>
#include <sys/resource.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
int main(int argc, char **argv) {
    if (argc < 3) return 125;
    struct timespec before, after;
    struct rusage usage;
    if (clock_gettime(CLOCK_MONOTONIC, &before)) return 125;
    pid_t child = fork();
    if (child < 0) return 125;
    if (!child) { execvp(argv[2], argv + 2); _exit(127); }
    int status;
    while (wait4(child, &status, 0, &usage) < 0) { if (errno != EINTR) return 125; }
    if (clock_gettime(CLOCK_MONOTONIC, &after)) return 125;
    FILE *output = fopen(argv[1], "w");
    if (!output) return 125;
    double elapsed = (after.tv_sec - before.tv_sec) * 1000.0 + (after.tv_nsec - before.tv_nsec) / 1000000.0;
    fprintf(output, "{\"elapsed_ms\":%.6f,\"peak_rss_bytes\":%lld}\n", elapsed, (long long)usage.ru_maxrss * 1024);
    if (fclose(output)) return 125;
    return WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
}
"""


def compact(value):
    return json.dumps(value, separators=(',', ':'), ensure_ascii=False)


def seed_history(spec, count):
    """Synthetic imported history only; never native execution or adoption evidence."""
    database = spec / '.dctx0ctl/state.sqlite3'
    with sqlite3.connect(database) as db:
        project, snapshot, check = db.execute('SELECT project,snapshot_id,id FROM checks ORDER BY id DESC LIMIT 1').fetchone()
        artifact = hashlib.sha256(b'{}').hexdigest()
        issue = {'code': 'SYNTHETIC_NOT_EXECUTED', 'message': 'Storage acceptance only', 'span': None}
        cases = []
        for i in range(count):
            result = {'status': ('PASS', 'FAIL', 'UNKNOWN')[i % 3]}
            if result['status'] != 'PASS':
                result['issue'] = issue
            cases.append({'id': f'C{i:05}', 'subject': 'HomeCinema.MovieNight.ReleaseYear', 'kind': 'contract',
                          'bindings': {'payload': {'kind': 'string', 'value': 'x' * (2097152 if i == 0 else 16384)}},
                          'setup': [], 'arguments': [], 'observations': [], 'events': None, 'states': [],
                          'result': result, 'elapsed_us': 0})
        required = [c['id'] for c in cases] + ['missing']
        evidence = {'schema_version': 1, 'origin': 'imported', 'started_at_unix_ns': '1', 'elapsed_us': 0,
                    'provenance': {'snapshot': snapshot, 'check_occurrence': check, 'plan_hash': artifact,
                                   'implementation': {'src/captured.ts': 'historical-capture'}, 'dependencies': {},
                                   'adapter_hash': '', 'configuration_hash': '', 'harness_hash': '',
                                   'interpreter_version': 'unexecuted', 'generator_version': 'synthetic-1',
                                   'protocol_version': 1, 'target': 'typescript', 'runtime_version': 'unexecuted',
                                   'build_configuration': {}, 'task_pin': None, 'incomplete': [issue]},
                    'budgets': {'seed': 42, 'generated_cases': 100, 'collection_size': 4, 'recursion_depth': 8,
                                'value_bytes': 65536, 'shrink_steps': 100, 'evaluation_steps': 100000},
                    'selected_subjects': ['HomeCinema.MovieNight.ReleaseYear'],
                    'required_subjects': ['HomeCinema.MovieNight.ReleaseYear'], 'not_selected': [],
                    'required_cases': required, 'cases': cases, 'open_obligations': [issue],
                    'bounded_obligations': ['Synthetic storage workload'],
                    'artifacts': [{'hash': artifact, 'media_type': 'application/json', 'bytes': 2}],
                    'result': {'status': 'FAIL', 'issue': issue}}
        raw = compact(evidence).encode()
        run_id = hashlib.sha256(raw).hexdigest()
        corpus = compact({'cases': cases, 'required_cases': required}).encode()
        corpus_hash = hashlib.sha256(corpus).hexdigest()
        started = time.perf_counter()
        packed = gzip.compress(corpus, mtime=0)
        compression_ms = (time.perf_counter() - started) * 1000
        header = {**evidence, 'cases': [], 'required_cases': []}
        db.execute('INSERT INTO verification_corpora VALUES(?,?,?)', (corpus_hash, packed, len(corpus)))
        db.execute('INSERT INTO verification_artifacts(hash,media_type,bytes,encoding,raw_length) VALUES(?,?,?,?,?)',
                   (artifact, 'application/json', b'{}', 'identity', 2))
        db.execute('INSERT INTO verification_runs(id,project,snapshot_id,check_id,origin,result,data,corpus_hash) VALUES(?,?,?,?,?,?,?,?)',
                   (run_id, project, snapshot, check, compact('imported'), compact(evidence['result']), compact(header), corpus_hash))
        db.execute('INSERT INTO verification_run_artifacts VALUES(?,?)', (run_id, artifact))
    return run_id, {'raw_corpus_bytes': len(corpus), 'legacy_compressed_bytes': len(packed),
                    'legacy_compression_ms': compression_ms, 'cases': count, 'payload_bytes': 16384,
                    'oversized_case_payload_bytes': 2097152}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--measure-only', action='store_true', help='use an already built release CLI; functional CLI assertions still run')
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--cases', type=int, default=512)
    args = parser.parse_args()
    os.environ.setdefault("DCTX0_TEST_PYTHON", sys.executable)
    if sys.platform != 'linux' or args.samples < 1 or args.cases < 3:
        parser.error('Linux wait4 RSS, positive samples and at least three cases required')
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    folder = ROOT / 'target/query-acceptance' / stamp
    folder.mkdir(parents=True)
    before = validate.digest(validate.validation_sources())
    report = {'schema_version': 1, 'timestamp': stamp, 'status': 'incomplete', 'checks': [], 'measurements': {},
              'source_hash_before': before, 'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': validate.capture(['git', 'status', '--porcelain']),
              'toolchain': validate.capture(['rustc', '--version']), 'platform': validate.capture(['uname', '-a']),
              'hardware': {'cpu': validate.capture(['lscpu']), 'cpu_count': os.cpu_count()},
              'settings': {'samples': args.samples, 'iterations': 1, 'warmup': 1, 'cache': 'warm OS cache',
                           'profile': 'release, uninstrumented', 'seed': 42},
              'memory_scope': 'Linux wait4 child ru_maxrss from a small native launcher; excludes Python fixture preparation. Wall time includes child fork/exec through reaping; no native compiler/harness in query workloads',
              'baseline': 'unassessed: new workloads; accepted baseline unchanged'}
    index = 0
    timer = folder / 'query-timer'

    def invoke(label, command, allowed=(0,), stdin=None, precise_rss=False):
        nonlocal index
        index += 1
        log = folder / f'{index:04}-{label.replace(chr(47), chr(45))}.log'
        rss_file = folder / f'{index:04}.rss'
        if precise_rss:
            command = [timer, rss_file, *command]
        if stdin is None:
            result = run([str(x) for x in command], log)
        else:
            import subprocess
            started = time.perf_counter()
            with log.open('wb') as output:
                process = subprocess.Popen([str(x) for x in command], stdin=subprocess.PIPE, stdout=output, stderr=subprocess.STDOUT, cwd=ROOT)
                process.stdin.write(stdin.encode())
                process.stdin.close()
                _, status, usage = os.wait4(process.pid, 0)
                result = {'command': list(map(str, command)), 'exit_code': os.waitstatus_to_exitcode(status),
                          'elapsed_ms': (time.perf_counter() - started) * 1000, 'peak_rss_bytes': usage.ru_maxrss * 1024,
                          'log': str(log.relative_to(ROOT))}
        if precise_rss:
            measured = json.loads(rss_file.read_text())
            peak = measured['peak_rss_bytes']
            result['launcher_elapsed_ms'] = result['elapsed_ms']
            result['elapsed_ms'] = measured['elapsed_ms']
            result['peak_rss_bytes'] = peak if peak > 0 else None
            result['memory_scope'] = report['memory_scope']
        report['checks'].append(result)
        if precise_rss and result['peak_rss_bytes'] is None:
            raise RuntimeError('CLI RSS is unavailable; measurements are incomplete')
        if result['exit_code'] not in allowed:
            raise RuntimeError(f'{label}: exit {result["exit_code"]}; see {log}')
        return result, log.read_text()

    def cli(label, *command, allowed=(0,), stdin=None):
        return invoke(label, [args.binary.resolve(), *command], allowed, stdin, precise_rss=True)

    def measure(label, command):
        cli(label + '-warmup', *command)
        samples = [cli(label, *command)[0] for _ in range(args.samples)]
        report['measurements'][label] = {'samples': samples,
                                        'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples),
                                        'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples), 'workload_version': 1}

    try:
        timer_source = folder / 'query-timer.c'
        timer_source.write_text(TIMER_SOURCE)
        compiler = shutil.which('cc')
        if not compiler:
            raise RuntimeError('C compiler unavailable; native CLI RSS launcher cannot be built')
        invoke('build-timer', [compiler, '-O2', '-Wall', '-Wextra', '-Werror', timer_source, '-o', timer])
        report['timer_source_sha256'] = hashlib.sha256(TIMER_SOURCE.encode()).hexdigest()
        report['timer_binary_sha256'] = hashlib.sha256(timer.read_bytes()).hexdigest()
        if not args.measure_only:
            for command in [['cargo', 'test', '-p', 'dctx0-store', '--all-features', '--locked'],
                            ['cargo', 'test', '-p', 'dctx0-engine', '--lib', '--all-features', '--locked'],
                            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'verification_discovery', '--all-features', '--locked'],
                            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'verification_run', '--all-features', '--locked', 'explicit_native_limits'],
                            ['cargo', 'test', '-p', 'dctx0-engine', '--test', 'native', '--all-features', '--locked', 'missing_runtime_invalid_limits'],
                            ['cargo', 'test', '-p', 'dctx0-cli', '--bin', 'dctx0', '--test', 'verification_history', '--test', 'verification_lifecycle', '--test', 'verification_inspection_large', '--test', 'verification_complete', '--test', 'verification_run', '--all-features', '--locked'],
                            ['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked']]:
                result, _ = invoke('check', command, allowed=range(256))
                if result['exit_code']:
                    report.setdefault('test_failures', []).append(result)
                if result['exit_code'] and command[1] == 'build':
                    raise RuntimeError('Release build failed; query measurements incomplete')
        report['binary_sha256'] = hashlib.sha256(args.binary.read_bytes()).hexdigest()
        report['binary_path'] = str(args.binary.resolve())
        with tempfile.TemporaryDirectory(prefix='query-', dir=folder) as temporary:
            for example in ('cinema', 'cinema-go'):
                spec = Path(temporary) / example
                shutil.copytree(ROOT / 'examples' / example / 'dctx0', spec, ignore=shutil.ignore_patterns('.dctx0ctl'))
                cli('check', 'check', spec, '--json')
                cli('tasks-plan', 'tasks', 'plan', spec, '--json')
                run_id, sizes = seed_history(spec, args.cases)
                report.setdefault('storage', {})[example] = sizes
                # Preparation is measured on fresh legacy corpus each repetition, not an idempotent no-op.
                dbpath = spec / '.dctx0ctl/state.sqlite3'
                legacy = Path(temporary) / (example + '-legacy.sqlite3')
                with sqlite3.connect(dbpath) as db, sqlite3.connect(legacy) as copy:
                    db.backup(copy)
                samples = []
                for repetition in range(args.samples + 1):
                    restored = Path(temporary) / f'{example}-prepare-{repetition}.sqlite3'
                    shutil.copyfile(legacy, restored)
                    sample, _ = cli('prepare', '--db', restored, 'query', 'prepare', spec, '--json')
                    if repetition:
                        samples.append(sample)
                report['measurements'][example + '/prepare'] = {'samples': samples, 'elapsed_ms': statistics.median(s['elapsed_ms'] for s in samples), 'peak_rss_bytes': max(s['peak_rss_bytes'] for s in samples), 'workload_version': 1}
                cli('prepare', 'query', 'prepare', spec, '--json')
                summary = json.loads(cli('summary', 'verify', 'show', spec, run_id, '--json')[1])
                assert summary['summary']['case_count'] == args.cases and summary['summary']['missing_case_count'] == 1
                assert 'cases' not in summary and len(compact(summary)) < 16384
                cases = json.loads(cli('filtered', 'verify', 'show', spec, run_id, '--outcome', 'PASS', '--limit', '5', '--json')[1])
                assert cases['summary']['result']['status'] == 'FAIL' and len(cases['cases']) == 5 and cases['detail'] is None
                rejected = cli('large-detail', 'verify', 'show', spec, run_id, '--case', 'C00000', '--json', allowed=(1,))[1]
                assert 'OUTPUT_LIMIT_EXCEEDED' in rejected
                exported = Path(temporary) / 'case.json'
                cli('case-export', 'verify', 'export', spec, run_id, '--case', 'C00000', '--output', exported)
                assert len(json.loads(exported.read_text())['bindings']['payload']['value']) == 2097152
                cli('schema', 'query', 'schema', spec, '--json')
                cli('stdin', 'query', 'sql', spec, '--file', '-', '--param', 'work="Open"', '--json', stdin='SELECT task_id,title,ready FROM dctx0_tasks WHERE work=:work ORDER BY wave,task_id')
                for sql in ['SELECT * FROM verification_runs', 'SELECT 1; SELECT 2', "ATTACH ':memory:' AS other", 'PRAGMA user_version']:
                    cli('unsafe', 'query', 'sql', spec, '--sql', sql, '--json', allowed=(1,))
                output = Path(temporary) / 'full.json'
                commands = {
                    'summary': ['verify', 'show', spec, run_id, '--json'],
                    'task-aggregate': ['query', 'sql', spec, '--sql', 'SELECT work,COUNT(*) FROM dctx0_tasks GROUP BY work ORDER BY work', '--json'],
                    'filtered-cases': ['verify', 'show', spec, run_id, '--outcome', 'FAIL,UNKNOWN', '--limit', '50', '--json'],
                    'single-case': ['verify', 'show', spec, run_id, '--case', 'C00001', '--json'],
                    'source-join': ['query', 'sql', spec, '--sql', 'SELECT t.task_id,s.subject,r.file,r.start_line FROM dctx0_tasks t JOIN dctx0_task_subjects s USING(task_id) JOIN dctx0_source_refs r ON r.subject=s.subject ORDER BY t.task_id,s.subject', '--json'],
                    'full-export': ['verify', 'export', spec, run_id, '--output', output]}
                for label, command in commands.items():
                    measure(example + '/' + label, command)
                full = json.loads(output.read_text())
                assert full['id'] == run_id and len(full['run']['cases']) == args.cases and full['run']['cases'][0]['id'] == 'C00000'
                with sqlite3.connect(dbpath) as db:
                    sizes['database_bytes'] = db.execute('PRAGMA page_count').fetchone()[0] * db.execute('PRAGMA page_size').fetchone()[0]
                    sizes['case_body_bytes'], sizes['case_raw_bytes'] = db.execute('SELECT SUM(length(bytes)),SUM(raw_length) FROM verification_case_bodies').fetchone()
                    sizes['metadata_bytes'] = db.execute('SELECT SUM(length(metadata)) FROM verification_case_index').fetchone()[0]
                    sizes['index_page_bytes'] = db.execute("SELECT SUM(pgsize) FROM dbstat WHERE name IN (SELECT name FROM sqlite_master WHERE type='index' AND tbl_name IN ('verification_case_index','verification_requirements','verification_case_projections'))").fetchone()[0]
                    assert db.execute('SELECT COUNT(*) FROM verification_corpora').fetchone()[0] == 0
                sizes['export_bytes'] = output.stat().st_size
        report['source_hash_after'] = validate.digest(validate.validation_sources())
        report['status'] = ('failed-checks' if report.get('test_failures') else 'complete') if before == report['source_hash_after'] else 'failed-source-drift'
    except Exception as error:
        report['error'] = str(error)
        print(error, file=sys.stderr)
    finally:
        import fcntl
        history = ROOT / 'benchmarks/history/queries'
        history.mkdir(parents=True, exist_ok=True)
        with (history / '.lock').open('a') as lock:
            fcntl.flock(lock, fcntl.LOCK_EX)
            target = history / f'{stamp}-{os.getpid()}.json'
            with tempfile.NamedTemporaryFile('w', dir=history, delete=False) as output:
                json.dump(report, output, indent=2)
                output.write('\n')
                output.flush()
                os.fsync(output.fileno())
            os.replace(output.name, target)
            for old in sorted(history.glob('*.json'))[:-20]:
                old.unlink()
        print('Query evidence:', target.relative_to(ROOT), flush=True)
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    raise SystemExit(main())
