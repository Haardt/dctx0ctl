#!/usr/bin/env python3
"""Real Rust self-verification acceptance, production mutations, then release speed/RSS."""
import argparse
import datetime
import hashlib
import fcntl
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time
import threading

ROOT = Path(__file__).resolve().parents[1]
HISTORY = ROOT / 'benchmarks/history/self-verification'


def measured(command, directory=ROOT):
    started = time.perf_counter()
    process = subprocess.Popen(command, cwd=directory, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    streams = [None, None]
    def read(index, stream):
        streams[index] = stream.read()
    readers = [threading.Thread(target=read, args=(index, stream)) for index, stream in enumerate((process.stdout, process.stderr))]
    for reader in readers: reader.start()
    _, status, usage = os.wait4(process.pid, 0)
    process.returncode = os.waitstatus_to_exitcode(status)
    for reader in readers: reader.join()
    stdout, stderr = streams
    return {'command': [str(item) for item in command], 'elapsed_ms': (time.perf_counter()-started)*1000,
            'peak_rss_bytes': usage.ru_maxrss * 1024, 'memory_method': 'Linux wait4 child-and-waited-descendants ru_maxrss; not incremental allocation',
            'exit_code': process.returncode, 'stdout': stdout.decode(), 'stderr': stderr.decode()}



def main():
    os.environ.setdefault('DCTX0_TEST_PYTHON', sys.executable)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT/'target/debug/dctx0')
    parser.add_argument('--checks', action='store_true', help='run the affected Rust regression tests first')
    parser.add_argument('--mutations', action='store_true', help='mutate only captured copies of real compiler/evidence production')
    args = parser.parse_args()
    binary = args.binary.resolve()
    report = {'schema_version': 1, 'timestamp': datetime.datetime.now(datetime.timezone.utc).isoformat(),
              'platform': platform.platform(), 'revision': subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),
              'dirty': bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),
              'toolchain': subprocess.check_output(['rustc','-vV'],text=True), 'seed':42,'generated_cases':16,'collection_size':2,
              'baseline': 'unassessed; no baseline promotion', 'checks':[], 'mutations':[], 'measurements':[], 'complete':False}
    try:
        if args.checks:
            for command in [
                ['cargo','build','-p','dctx0-cli','--all-features','--locked'],
                ['cargo','fmt','--all','--','--check'],
                ['cargo','clippy','--workspace','--all-targets','--all-features','--locked','--','-D','warnings'],
                ['cargo','test','-p','dctx0-engine','--lib','--all-features','--locked'],
                ['cargo','test','-p','dctx0-engine','--test','native','--test','verification_rust','--test','tasks','--test','profiles','--all-features','--locked'],
                ['cargo','test','-p','dctx0-cli','--no-default-features','--locked'],
                ['cargo','test','-p','dctx0-lsp','-p','dctx0-cli','--lib','--test','self_verification','--test','lsp','--all-features','--locked'],
            ]:
                result=measured(command);report['checks'].append(result)
                if result['exit_code']:raise RuntimeError(f"Regression check failed: {' '.join(command)}\n{result['stderr'][-4000:]}")
        functional=measured([str(binary),'verify','self',str(ROOT),'--json'])
        report['checks'].append({key:value for key,value in functional.items() if key!='stdout'})
        if not functional['stdout'].strip():
            raise RuntimeError(f"Self-verification produced no report (exit {functional['exit_code']}): {functional['stderr'][-4000:]}")
        self_report=json.loads(functional['stdout'])
        report['self_report']=self_report
        report['source_hash']=hashlib.sha256(json.dumps(self_report['source_files'],sort_keys=True).encode()).hexdigest()
        report['hardware']={'architecture':platform.machine(),'cpu_count':os.cpu_count(),'cpu_model':next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),None),'memory_bytes':next((int(line.split()[1])*1024 for line in Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemTotal:')),None)}
        if functional['exit_code'] or not self_report['machine_complete']:raise RuntimeError('Mandatory self-verification is incomplete')
        captured=Path(self_report['captured_root'])
        captured_cli=captured/'.dctx0/testing/rust/executables/dctx0'
        if args.mutations:
            mutations=[
                ('compiler',captured/'crates/dctx0-language/src/compiler.rs','Some(ir)','None','Dctx0.Compiler'),
                ('evidence',captured/'crates/dctx0-tasks/src/adoption.rs','requirements.is_empty()\n        || coverage.required_cases.is_empty()','false','Dctx0.Evidence'),
            ]
            for name,path,needle,replacement,subject in mutations:
                original=path.read_text()
                if needle not in original:raise RuntimeError(f'Mutation anchor absent: {name}')
                try:
                    path.write_text(original.replace(needle,replacement,1))
                    # Native run builds the worker against this captured real production mutation.
                    result=measured([str(captured_cli),'verify','run',str(captured/'dctx0'),'--project',str(captured),'--subject',subject,'--no-adopt','--json'])
                    evidence=json.loads(result['stdout'])
                    failures=[case['id'] for case in evidence['evidence']['run']['cases'] if case['result']['status']=='FAIL']
                    report['mutations'].append({'name':name,'source':str(path.relative_to(captured)),'failures':failures,'result':{key:value for key,value in result.items() if key!='stdout'}})
                    if not failures:raise RuntimeError(f'Production mutation escaped: {name}')
                finally:path.write_text(original)
        # Performance runs last. Preparation and orchestration RSS exclude child compilers.
        report['measurements'].append({'phase':'preparation','elapsed_ms':self_report['preparation_ms'],'peak_rss_bytes':self_report['preparation_peak_rss_bytes'],'memory_method':'post-preparation orchestrator /proc VmHWM; includes preparation','passed':True})
        cold=measured(['cargo','build','--release','--offline','--locked','--all-features','-p','dctx0-cli','--target-dir',str(captured/'.bin/cold-cli-build')],captured)
        report['measurements'].append({'phase':'cold-cli-build','cache_condition':'fresh target directory; existing registry cache','passed':cold['exit_code']==0,**cold})
        if cold['exit_code']:raise RuntimeError('Cold release CLI build failed')
        report['measurements'].append({'phase':'orchestrator','elapsed_ms':self_report['execution_ms'],'peak_rss_bytes':self_report['orchestrator_peak_rss_bytes'],'memory_method':'post-execution orchestrator /proc VmHWM; previous stages included, child workers excluded','passed':True})
        report['measurements'].append({'phase':'native-worker','samples':self_report['runtime_measurements'],'memory_method':'worker-reported /proc VmHWM, captured in native artifacts','passed':True})
        replay_source = None
        for phase in ('warm-finite-plan', 'replay'):
            command = ([str(captured_cli),'verify','run',str(captured/'dctx0'),'--project',str(captured),'--no-adopt','--json'] if phase == 'warm-finite-plan' else [str(captured_cli),'verify','replay',str(captured/'dctx0'),replay_source[0],'--project',str(captured),'--case',replay_source[1],'--json'])
            # Replay the most recent complete corpus: mutation and warm runs
            # legitimately expire the original under the three-run policy.

            result=measured(command)
            if not result['stdout'].strip():
                raise RuntimeError(f"{phase} produced no JSON (exit {result['exit_code']}): {result['stderr'][-4000:]}")
            evidence=json.loads(result['stdout'])
            if phase=='warm-finite-plan':
                cases=evidence['evidence']['run']['cases'];passed=bool(cases) and all(case['result']['status']=='PASS' for case in cases) and not evidence['evidence']['run']['provenance']['incomplete']
                if passed: replay_source = (evidence['evidence']['id'], cases[0]['id'])
            else:passed=evidence['case_result']['status']=='PASS'
            report['measurements'].append({'phase':phase,'passed':passed,**{key:value for key,value in result.items() if key!='stdout'}})
            if not passed:raise RuntimeError(f'Performance execution incomplete: {phase}')
        report['complete']=True
    except Exception as error:
        report['error']=str(error)
    finally:
        HISTORY.mkdir(parents=True,exist_ok=True)
        with (HISTORY/'.lock').open('a') as lock:
            fcntl.flock(lock,fcntl.LOCK_EX)
            filename=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+f'-{os.getpid()}.json'
            temporary=HISTORY/(filename+'.tmp')
            temporary.write_text(json.dumps(report,indent=2)+'\n');temporary.replace(HISTORY/filename)
            for path in sorted(HISTORY.glob('*.json'))[:-20]:path.unlink()
        print(json.dumps({'complete':report['complete'],'report':str(HISTORY/filename),'error':report.get('error')}))
    return 0 if report['complete'] else 1

if __name__=='__main__':raise SystemExit(main())
