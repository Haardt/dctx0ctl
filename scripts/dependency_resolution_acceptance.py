#!/usr/bin/env python3
"""Real Bun dependency graphs, coupled to release speed/RSS measurements; no adoption."""
from verification_storage import saved_verification_artifacts
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
import tempfile

from release_support import locked, measured
from validate import atomic, digest, validation_sources

ROOT = Path(__file__).resolve().parents[1]
HISTORY = ROOT / 'benchmarks/history/dependency-resolution'
MEDIA = 'application/vnd.dctx0.'
FACTORY = "export function factory(value:number){if(!Number.isInteger(value)||value<1||value>2)throw TypeError('invalid');return value;}"
ADAPTER = """import {factory} from '../../../app.ts';
export const adapter={async describe(){return {protocol_version:1,subjects:[{subject:'W.A',role:'contract',artifact:'app.ts',entry_point:'factory'}],artifacts:['app.ts'],capabilities:['sessions','values','events'],representations:{Int:'safe integer'}};},async begin(){return {async invoke(){throw Error('No operation');},async validate(subject,value){let result;try{result=factory(value.kind==='integer'||value.kind==='float'?Number(value.value):value.kind==='unit'?undefined:value.value);}catch(error){return {kind:'rejected',reason:String(error)};}return {kind:'accepted',value:{kind:'integer',value:String(result)}};},async close(){}};}};"""


def run(command, cwd):
    result = subprocess.run(list(map(str, command)), cwd=cwd, text=True, capture_output=True)
    if result.returncode:
        raise RuntimeError(f'{command}: {result.stderr[-4000:]} {result.stdout[-1000:]}')
    return result.stdout


def artifacts(spec, report):
    rows = saved_verification_artifacts(spec / ".dctx0ctl/state.sqlite3", report['evidence']['id'])
    result = {}
    for media, raw, expected in rows:
        if hashlib.sha256(raw).hexdigest() != expected:
            raise RuntimeError('Artifact digest mismatch')
        if media in [MEDIA + suffix + '+json' for suffix in ('dependency-resolution-limits', 'dependency-resolution-summary', 'runtime-metrics')]:
            result[media] = json.loads(raw)
    return result


def prepare(binary, root, kind):
    app = root / kind
    spec = app / 'spec'
    spec.mkdir(parents=True)
    (spec / 'w.dctx0').write_text('workspace W { value A = Int { range 1..2 } }\n')
    run([binary, 'check', spec, '--json'], ROOT)
    run([binary, 'verify', 'init', spec, '--project', app, '--target', 'typescript', '--subject', 'W.A', '--json'], ROOT)
    (app / '.dctx0/testing/typescript/adapter.ts').write_text(ADAPTER)
    if kind == 'chain':
        (app / 'chain').mkdir()
        for index in range(1200):
            following = 600 if index == 1199 else index + 1
            (app / f'chain/{index}.ts').write_text(f"import './{following}.ts'; export {{}};\n")
        imports = "import './chain/0.ts';\n"
    else:
        (app / 'helper.ts').write_text('export const helper = 1;\n')
        imports = "import './helper.ts';\n" * 1200
    (app / 'app.ts').write_text(imports + FACTORY)
    if kind == 'budget-stop':
        path = app / '.dctx0/testing/verification.json'
        config = json.loads(path.read_text())
        config['dependency_resolution'] = {'max_references': 16}
        path.write_text(json.dumps(config))
    return app, spec


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    args = parser.parse_args()
    binary = args.binary.resolve()
    now = datetime.now(timezone.utc)
    stamp = now.strftime('%Y%m%dT%H%M%S.%fZ') + f'-{os.getpid()}'
    record = {'schema_version': 1, 'id': stamp, 'timestamp': now.isoformat(), 'status': 'incomplete',
              'metadata': {'revision': run(['git', 'rev-parse', 'HEAD'], ROOT).strip(),
                           'dirty_worktree': bool(run(['git', 'status', '--porcelain'], ROOT)),
                           'source_hash_before': digest(validation_sources()),
                           'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
                           'platform': platform.platform(), 'machine': platform.machine(), 'logical_cpus': os.cpu_count(),
                           'rust': run(['rustc', '-vV'], ROOT).strip(), 'bun': run(['bun', '--version'], ROOT).strip()},
              'workload': {'references': 1200, 'cycle_back_to': 600, 'seed': 42, 'generated_cases': 100,
                           'features': 'all-features', 'build': 'release without coverage', 'warmups': 1, 'samples': 3,
                           'cache': 'prebuilt CLI and Bun; one unrecorded warmup per fixture; fresh capture each run; warm OS filesystem cache'},
              'baseline': {'status': 'unassessed', 'reason': 'New workloads have no accepted baseline; existing baselines unchanged'},
              'workloads': {}}
    failure = None
    try:
        # Owned scratch stays on disk; no production fixture/history is modified.
        scratch_root = ROOT / 'target/dependency-resolution'
        scratch_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=scratch_root) as temporary:
            for kind in ('chain', 'repeated', 'budget-stop'):
                app, spec = prepare(binary, Path(temporary), kind)
                samples = []
                record['workloads'][kind] = {'raw_samples': samples}
                for index in range(4):
                    sample, output = measured([str(binary), 'verify', 'run', str(spec), '--project', str(app), '--no-adopt', '--json', '--full'], ROOT, timeout=300, sample_direct_rss=True)
                    report = json.loads(output)
                    details = artifacts(spec, report)
                    summary = details[MEDIA + 'dependency-resolution-summary+json']
                    limits = details[MEDIA + 'dependency-resolution-limits+json']
                    metrics = details[MEDIA + 'runtime-metrics+json']
                    issues = report['evidence']['run']['provenance']['incomplete']
                    budget_issues = [issue for issue in issues if issue['code'] == 'DEPENDENCY_RESOLUTION_BUDGET']
                    if report['evidence']['run']['result']['status'] != 'PASS' or report['adopted_tasks']:
                        raise RuntimeError(f'{kind}: native cases failed or tasks adopted')
                    if kind == 'budget-stop':
                        if not budget_issues or summary['processed_references'] != 16 or summary['traversal_complete'] or limits['max_references'] != 16:
                            raise RuntimeError('Budget stop did not preserve its exact prefix')
                    elif issues or not summary['traversal_complete'] or summary['processed_references'] < 1200:
                        raise RuntimeError(f'{kind}: incomplete dependency evidence: {issues}')
                    if kind == 'repeated' and summary['cache_hits'] < 1199:
                        raise RuntimeError('Repeated references were not cached with full evidence')
                    sample.update(engine_elapsed_ms=report['evidence']['run']['elapsed_us'] / 1000,
                                  worker_peak_rss_bytes=metrics['peak_rss_bytes'],
                                  worker_memory_scope='Bun post-exec /proc/self/status VmHWM; unavailable off Linux',
                                  dependency_summary=summary, limits=limits,
                                  native_cases=len(report['evidence']['run']['cases']))
                    if index:
                        samples.append(sample)
                    print(f'{kind} {"warmup" if not index else index}: {sample["elapsed_ms"]:.1f} ms, command RSS {sample["peak_rss_bytes"]}, worker RSS {sample["worker_peak_rss_bytes"]}', flush=True)
                record['workloads'][kind].update(
                    median_elapsed_ms=statistics.median(s['elapsed_ms'] for s in samples),
                    median_peak_rss_bytes=statistics.median(s['peak_rss_bytes'] for s in samples),
                    median_engine_peak_rss_bytes=statistics.median(s['direct_process_peak_rss_bytes'] for s in samples) if all(s['direct_process_peak_rss_bytes'] is not None for s in samples) else None,
                    median_worker_peak_rss_bytes=statistics.median(s['worker_peak_rss_bytes'] for s in samples) if all(s['worker_peak_rss_bytes'] is not None for s in samples) else None)
        record['status'] = 'complete'
    except (OSError, ValueError, RuntimeError, TimeoutError, KeyError) as error:
        failure = str(error)
        record.update(status='failed', failure=failure)
    finally:
        record['metadata']['source_hash_after'] = digest(validation_sources())
        if record['metadata']['source_hash_after'] != record['metadata']['source_hash_before']:
            failure = 'Source inputs changed during acceptance'
            record.update(status='failed', failure=failure)
        HISTORY.mkdir(parents=True, exist_ok=True)
        with locked(HISTORY.parent / '.dependency-resolution.lock'):
            atomic(HISTORY / (stamp + '.json'), record)
            for old in sorted(HISTORY.glob('*.json'))[:-20]:
                old.unlink()
    print(json.dumps({'status': record['status'], 'report': str(HISTORY / (stamp + '.json')), 'failure': failure}))
    return 1 if failure else 0


if __name__ == '__main__':
    from memory_guard import guard_script
    guard_script()
    raise SystemExit(main())
