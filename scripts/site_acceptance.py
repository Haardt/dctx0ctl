#!/usr/bin/env python3
"""Cinema read-only companion acceptance, paired with release speed/RSS samples."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import signal
import statistics
import sqlite3
import subprocess
import sys
import tempfile
import time
from release_support import measured, locked
ROOT=Path(__file__).resolve().parents[1]
HISTORY=ROOT/'benchmarks/history/site'

def inputs():
    from validate import validation_sources,digest
    return digest([*validation_sources(),*ROOT.glob('dctx0/*.dctx0'),*ROOT.glob('.dctx0/testing/rust/*.rs'),ROOT/'.dctx0/testing/rust/Cargo.toml',ROOT/'.dctx0/testing/rust/Cargo.lock'])

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--samples',type=int,default=3)
    parser.add_argument('--iterations',type=int,default=5)
    args=parser.parse_args()
    if args.samples<1 or args.iterations<1:parser.error('Positive samples and iterations required')
    report={'schema_version':1,'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'passed':False,'complete':False,'source_hash':inputs(),'revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'dirty':bool(subprocess.check_output(['git','status','--porcelain'],cwd=ROOT,text=True)),'platform':platform.platform(),'hardware':{'cpu_count':os.cpu_count(),'machine':platform.machine()},'baseline_status':'unassessed: no accepted site workload baseline','samples':args.samples,'iterations':args.iterations,'measurements':{},'checks':[],'workload':'isolated unchanged Cinema spec; explicit test history; release CLI and embedded UI','cache_conditions':'one warm-up before release phase samples; existing Cargo/Bun caches; fresh isolated SQLite history'}
    def run(label,command,allowed=(0,),timeout=1200):
        sample,output=measured(list(map(str,command)),ROOT,timeout=timeout,env={**os.environ,"CARGO_TARGET_DIR":str(ROOT/"target/site/build")})
        report['measurements'].setdefault(label,[]).append(sample)
        logs=ROOT/'target/site/logs';logs.mkdir(parents=True,exist_ok=True);(logs/(label+'.log')).write_text(output)
        if sample['exit_code'] not in allowed:raise RuntimeError(label+': '+output[-5000:])
        return output
    report['toolchain']={name:subprocess.check_output(command,cwd=ROOT,text=True).strip() for name,command in {'rustc':['rustc','--version'],'cargo':['cargo','--version'],'bun':['bun','--version']}.items()}
    report['hardware']['cpu_model']=next((line.split(':',1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines() if line.startswith('model name')),None) if sys.platform=='linux' else None
    report['spec_input_hash']=hashlib.sha256(b''.join(p.read_bytes() for p in sorted((ROOT/'examples/cinema/dctx0').glob('*.dctx0')))).hexdigest()
    report['build_target']='target/site/build (isolated from workspace feature jobs)'
    child=None
    try:
        run('ui-typecheck',['bun','run','--cwd','ui/site','check'])
        run('ui-unit',['bun','test','--cwd','ui/site','src/model.test.ts'])
        run('ui-reproducibility',['bun','run','--cwd','ui/site','verify-assets'])
        run('rust-site-engine',['cargo','test','-p','dctx0-engine','--test','site','--all-features','--locked'])
        run('rust-site-cli',['cargo','test','-p','dctx0-cli','--test','site','--all-features','--locked'])
        run('rust-site-http-errors',['cargo','test','-p','dctx0-cli','--bin','dctx0','site::','--all-features','--locked'])
        run('release-build',['cargo','build','--release','-p','dctx0-cli','--all-features','--locked'])
        binary=ROOT/'target/site/build/release/dctx0'
        with tempfile.TemporaryDirectory(prefix='acceptance-',dir=ROOT/'target/site') as tmp:
            tmp=Path(tmp);app=tmp/'cinema';shutil.copytree(ROOT/'examples/cinema',app,ignore=shutil.ignore_patterns('node_modules','.dctx0ctl','target','.bin','cinema.sqlite3*'))
            shutil.copytree(ROOT/'examples/cinema/node_modules',app/'node_modules')
            spec=app/'dctx0';out=tmp/'export'
            def cli(label,*command,allowed=(0,)):return run(label,[binary,'--json',*command],allowed=allowed)
            cli('isolated-check','check',spec);plan=json.loads(cli('isolated-plan','tasks','plan',spec))['body']['data'];
            # Actual bounded native contract run, not synthesized browser evidence.
            native=json.loads(cli('native-cinema-contract','verify','run',spec,'--project',app,'--subject','HomeCinema.MovieNight.ReleaseYear','--no-adopt',allowed=(0,1))); assert native['evidence']['run']['cases']
            # Real application mutation produces stored FAIL; repairing it allows replay.
            source=app/'src/domain/release-year.ts';original=source.read_text();assert 'input < 1888 || input > 2100' in original
            source.write_text(original.replace('input < 1888 || input > 2100','input < 1889 || input > 2100'))
            failed=json.loads(cli('native-cinema-mutation','verify','run',spec,'--project',app,'--subject','HomeCinema.MovieNight.ReleaseYear','--no-adopt',allowed=(1,)))
            failed_case=next(case for case in failed['evidence']['run']['cases'] if case['result']['status']=='FAIL')
            source.write_text(original)
            cli('native-cinema-replay','verify','replay',spec,failed['evidence']['id'],'--case',failed_case['id'],'--project',app,allowed=(0,1))
            report['checks'].append('actual bounded Cinema native PASS, production mutation FAIL, repaired replay; provenance limits retained')
            out.mkdir();(out/'unmanaged.txt').write_text('keep')
            database=spec/'.dctx0ctl/state.sqlite3'
            def fingerprint():
                with sqlite3.connect(f'file:{database}?mode=ro',uri=True) as conn:return hashlib.sha256('\n'.join(conn.iterdump()).encode()).hexdigest()
            before=fingerprint();sources={str(p.relative_to(spec)):hashlib.sha256(p.read_bytes()).hexdigest() for p in spec.glob('*.dctx0')}
            cli('export-warmup','site','export',spec,'--output',out)
            first=(out/'index.html').read_bytes()
            for i in range(args.samples):
                raw=json.loads(cli('snapshot-projection','bench',spec,'site-snapshot',args.iterations));report['measurements']['snapshot-projection'][-1]['rust_phase']=raw
                cli('offline-export','site','export',spec,'--output',out)
            assert first==(out/'index.html').read_bytes();assert (out/'unmanaged.txt').read_text()=='keep';assert fingerprint()==before
            report['checks'].append('deterministic export preserves unmanaged files and published database')
            server_started=time.perf_counter()
            child=subprocess.Popen([str(binary),'site','serve',str(spec)],cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            url=child.stdout.readline().strip();assert url.startswith('http://127.0.0.1:')
            browser_report=tmp/'browser.json'
            run('browser-checks',['bun','ui/site/browser.ts',out,url,binary,spec,browser_report],timeout=180)
            report['browser']=json.loads(browser_report.read_text());assert report['browser']['passed'];
            run('architecture-focus-browser',['bun','ui/site/focus-browser.ts',out],timeout=180)
            report['checks'].append('architecture subtree focus, depth, hash navigation and small viewport')
            for screenshot in ('cinema-site.png','architecture-graph.png','task-graph.png','architecture-focus.png'):shutil.copyfile(out/screenshot,ROOT/'target/site'/screenshot)
            assert sources=={str(p.relative_to(spec)):hashlib.sha256(p.read_bytes()).hexdigest() for p in spec.glob('*.dctx0')};report['checks'].append('Cinema sources unchanged through browser transitions')
            # Explicit relevant change, follow-up and rename in this isolated test history only.
            file=next(p for p in spec.glob('*.dctx0') if 'range 1888' in p.read_text());text=file.read_text();assert '1888..2100' in text;file.write_text(text.replace('1888..2100','1889..2100'))
            cli('changed-check','check',spec);cli('changed-plan','tasks','plan',spec);cli('rename-check','rename',spec,'HomeCinema.MovieNight.ReleaseYear','ReleaseYearInput');cli('rename-plan','tasks','plan',spec)
            cli('changed-export','site','export',spec,'--output',out);view=json.loads((out/'snapshot.json').read_text());assert view['records']['renames'];assert view['records']['plan_diff'];assert any(d['changes'] for d in view['records']['tasks']['definitions'].values());report['checks'].append('real semantic change and explicit rename retain follow-up history')
            cli('historical-export','site','export',spec,'--output',tmp/'historical','--snapshot',plan['snapshot_id'],'--plan',str(plan['id']))
            status=Path(f'/proc/{child.pid}/status').read_text() if sys.platform=='linux' else ''
            server_rss=next((int(line.split()[1])*1024 for line in status.splitlines() if line.startswith('VmHWM:')),None)
            report['measurements']['server-process']=[{'elapsed_ms':(time.perf_counter()-server_started)*1000,'peak_rss_bytes':server_rss,'memory_scope':'Linux server process VmHWM across warm-up, requests and coherent refreshes; excludes browser and CLI workers','memory_status':'measured' if server_rss is not None else 'unavailable: process VmHWM not supported','exit_code':0}]
            os.killpg(child.pid,signal.SIGINT);assert child.wait(timeout=10)==0;child=None
        report['cli_sha256']=hashlib.sha256(binary.read_bytes()).hexdigest()
        report['assets']=json.loads((ROOT/'ui/site/dist/provenance.json').read_text());report['bundle_bytes']={name:(ROOT/'ui/site/dist'/name).stat().st_size for name in ('site.js','site.css')}
        report['source_hash_after']=inputs();assert report['source_hash_after']==report['source_hash'];report['passed']=True;report['complete']=True
    except Exception as error:
        report['error']=repr(error);import traceback;traceback.print_exc()
    finally:
        if child is not None:os.killpg(child.pid,signal.SIGINT);child.wait(timeout=10)
        report['summary']={phase:{'samples':len(samples),'elapsed_ms_median':statistics.median(s['elapsed_ms'] for s in samples),'peak_rss_bytes_median':statistics.median(s['peak_rss_bytes'] for s in samples if s['peak_rss_bytes'] is not None) if any(s['peak_rss_bytes'] is not None for s in samples) else None} for phase,samples in report['measurements'].items()}
        report['finished_at']=datetime.datetime.now(datetime.timezone.utc).isoformat()
        HISTORY.mkdir(parents=True,exist_ok=True)
        from validate import atomic
        with locked(HISTORY/'.lock'):
            filename=datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')+f'-{os.getpid()}.json'
            previous=[json.loads(p.read_text()) for p in sorted(HISTORY.glob('*.json')) if p.is_file()]
            comparable=[r for r in previous if r.get('complete') and all(r.get(k)==report.get(k) for k in ('platform','hardware','toolchain','spec_input_hash','samples','iterations','cache_conditions'))]
            report['recent_trends']=[{'started_at':r['started_at'],'summary':r.get('summary',{})} for r in comparable[-5:]]
            report['comparison_status']='compatible history available; no accepted site baseline' if comparable else 'no comparable complete history; unassessed'
            atomic(HISTORY/filename,report)
            for old in sorted(HISTORY.glob('*.json'))[:-20]:old.unlink()
            (ROOT/'target/site').mkdir(parents=True,exist_ok=True);atomic(ROOT/'target/site/acceptance.json',report)
        print(json.dumps({'passed':report['passed'],'complete':report['complete'],'record':str(HISTORY/filename),'baseline':report['baseline_status']}))
    return 0 if report['passed'] else 1
if __name__=='__main__':
    from memory_guard import guard_script
    guard_script()
    sys.exit(main())
