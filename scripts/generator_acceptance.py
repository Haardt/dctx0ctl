#!/usr/bin/env python3
"""Execute three-target generated tests and retain release CLI speed/RSS evidence."""
import history_retention
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile
from release_support import locked, measured
import validate

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--scaffolds-only', action='store_true', help='partial scaffold acceptance; workspace caller supplies codec/engine regressions')
    parser.add_argument('--skip-build', action='store_true', help='use an already built release CLI')
    args = parser.parse_args()
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime('%Y%m%dT%H%M%S.%fZ')
    output = ROOT / 'target/generator-acceptance' / stamp
    output.mkdir(parents=True)
    before = validate.digest(validate.validation_sources())
    report = {'schema_version': 1, 'timestamp': stamp, 'status': 'incomplete',
              'revision': validate.capture(['git', 'rev-parse', 'HEAD']),
              'dirty_worktree': bool(validate.capture(['git', 'status', '--porcelain'])),
              'source_hash_before': before, 'platform': platform.platform(),
              'hardware': {'machine': platform.machine(), 'cpu_count': os.cpu_count()},
              'toolchains': {name: validate.capture(command) for name, command in [
                  ('rust', ['rustc', '-vV']), ('go', ['go', 'version']), ('bun', ['bun', '--version'])]},
              'settings': {'features': 'all-features', 'seed': 42, 'samples': 1,
                           'cache': 'existing compiler caches, fresh isolated projects',
                           'profile': 'uninstrumented release CLI; native test commands measured separately'},
              'scope': 'scaffolds only' if args.scaffolds_only else 'codecs, publication and three-target central scaffolds',
              'baseline': {'status': 'unassessed', 'reason': 'new generator workloads; no baseline promotion'},
              'measurements': []}
    environment = {**os.environ, 'GOTOOLCHAIN': 'local', 'GOWORK': 'off', 'GOPROXY': 'off',
                   'CGO_ENABLED': '0', 'DCTX0_TEST_CLI': str(args.binary.resolve())}

    def run(label, command, cwd=ROOT, expected=0):
        print(label, flush=True)
        sample, text = measured(list(map(str, command)), cwd, timeout=1800, env=environment)
        (output / f'{label}.log').write_text(text)
        report['measurements'].append({'label': label, 'command': list(map(str, command)),
                                       'expected_exit': expected, **sample})
        if (sample['exit_code'] == 0) != (expected == 0):
            raise RuntimeError(f'{label} returned {sample["exit_code"]}; see {output}')
        return text

    try:
        if not args.skip_build:
            run('release-build', ['cargo', 'build', '--release', '-p', 'dctx0-cli', '--all-features', '--locked'])
        binary = args.binary.resolve()
        report['binary_hash'] = hashlib.sha256(binary.read_bytes()).hexdigest()
        if not args.scaffolds_only:
            run('native-codecs', ['cargo', 'test', '-p', 'dctx0-target-go', '-p', 'dctx0-target-rust',
                                  '-p', 'dctx0-target-typescript', '--test', 'codecs', '--locked'])
            run('publication-and-central-driver', ['cargo', 'test', '-p', 'dctx0-engine', '--test',
                                                 'codec_scaffold', '--test', 'verification_go', '--locked'])
        if not args.scaffolds_only:
            run('generator-cli', ['cargo','test','-p','dctx0-cli','--test','generators','--all-features','--locked'])
        for target in ('go', 'typescript', 'rust'):
            app = output / f'{target} application with spaces'
            spec = app / 'spec'
            spec.mkdir(parents=True)
            (spec / 'w.dctx0').write_text('workspace W { value Count = Int { range 1..2 } surface Api { operation next(input:Count) -> Int {\n pure\n ensures result == input + 1\n } } }\n')
            if target == 'go':
                (app / 'production').mkdir()
                (app / 'go.mod').write_text('module fixture\n\ngo 1.25.0\n')
                production = app / 'production/next.go'
            elif target == 'typescript':
                (app / 'package.json').write_text('{"type":"module"}\n')
                production = app / 'production.ts'
            else:
                (app / 'src').mkdir()
                (app / 'Cargo.toml').write_text('[package]\nname="fixture"\nversion="0.1.0"\nedition="2024"\n[workspace]\n[dependencies]\nnum-bigint="=0.4.6"\nserde_json={version="=1.0.151",features=["arbitrary_precision"]}\n')
                production = app / 'src/lib.rs'
            suffix = {'go': 'go', 'typescript': 'ts', 'rust': 'rs'}[target]
            fixture = ROOT / 'fixtures/conformance/generator' / target
            shutil.copyfile(fixture / f'production.{suffix}', production)
            run(f'{target}-check', [binary, 'check', spec])
            run(f'{target}-init', [binary, 'verify', 'init', spec, '--project', app, '--target', target, '--subject', 'W.Api.next'])
            directory = app / '.dctx0/testing' / target
            shutil.copyfile(fixture / f'adapter.{suffix}', directory / f'adapter.{suffix}')
            if target == 'rust':
                manifest = directory / 'Cargo.toml'
                manifest.write_text(manifest.read_text() + '\nfixture={path="../../.."}\nnum-bigint="=0.4.6"\n')
                run('rust-prepare', ['cargo', 'generate-lockfile', '--offline'], directory)
            command = [binary, 'verify', 'bindings', 'scaffold', spec, '--project', app, '--target', target, '--subject', 'W.Api.next']
            proposal = json.loads(run(f'{target}-proposal', command + ['--dry-run', '--json']))
            assert not (directory / proposal[0]['artifact']['path']).exists()
            assert proposal == json.loads(run(f'{target}-deterministic-proposal', command + ['--dry-run', '--json']))
            run(f'{target}-generate', command)
            run(f'{target}-candidate', command + ['--candidate', 'upgrade'])
            entry = directory / {'go': 'contract_plan_test.go', 'typescript': 'contract-plan.test.ts', 'rust': 'contract_plan_tests.rs'}[target]
            original = entry.read_bytes()
            run(f'{target}-preserve', command)
            assert entry.read_bytes() == original
            if target == 'go':
                test = ['go', 'test', '-count=1', '-timeout=120s', '-run=^TestPublishedContractPlan$', '.']
            elif target == 'typescript':
                test = ['bun', 'test', '--timeout', '120000', 'contract-plan.test.ts']
            else:
                test = ['cargo', 'test', '--offline', '--locked', '--lib', 'published_contract_plan']
            run(f'{target}-unimplemented-hook', test, directory, expected=1)
            authored = entry.read_text()
            if target == 'go':
                authored = authored.replace('\t"fmt"\n', '\t"os"\n').replace('return nil, fmt.Errorf("production bindings and fixture prerequisites have not been reviewed")', 't.Cleanup(func(){ if err:=os.WriteFile("fixture-cleanup.receipt",[]byte("closed"),0600);err!=nil{t.Error(err)} }); return map[string]*string{}, nil')
            elif target == 'typescript':
                authored = authored.replace("throw new Error('Production bindings and fixture prerequisites have not been reviewed');", 'return {environment:{},async close(){await Bun.write("fixture-cleanup.receipt","closed");}};')
            else:
                authored = authored.replace('Err("production bindings and fixture prerequisites have not been reviewed".into())', 'Ok(support::Fixture{environment:Default::default(),cleanup:Some(Box::new(||std::fs::write("fixture-cleanup.receipt","closed").expect("cleanup")))})')
            assert authored != entry.read_text()
            entry.write_text(authored)
            history = spec / '.dctx0ctl/state.sqlite3'
            saved = history.read_bytes()
            run(f'{target}-actual-production', test, directory)
            assert history.read_bytes() == saved
            receipt = directory / 'fixture-cleanup.receipt'
            assert receipt.read_text() == 'closed'
            receipt.unlink()
            actual = production.read_text()
            broken = actual.replace('big.NewInt(1)', 'big.NewInt(2)').replace('input+1n', 'input+2n').replace('input+1}', 'input+2}')
            assert broken != actual
            production.write_text(broken)
            run(f'{target}-production-mutation', test, directory, expected=1)
            assert history.read_bytes() == saved
            assert receipt.read_text() == 'closed'
            receipt.unlink()
            # A permissive real production validator must fail generated negative cases.
            permissive=actual.replace('return ok && exact.Cmp(big.NewInt(1)) >= 0 && exact.Cmp(big.NewInt(2)) <= 0','return ok && exact != nil').replace("typeof value==='bigint'&&value>=1n&&value<=2n", "true").replace('value.as_i64().is_some_and(|n|(1..=2).contains(&n))','true')
            assert permissive != actual
            production.write_text(permissive)
            run(f'{target}-permissive-validator-mutation',test,directory,expected=1)
            assert history.read_bytes()==saved
            assert receipt.read_text()=='closed'
            receipt.unlink()
            # Preserve a wrong primitive all the way into production. An adapter
            # that wraps it in a wire object would hide this narrower mutation.
            accepts_string = actual.replace(
                '\tif !ok {\n\t\treturn false\n\t}',
                '\tif !ok {\n\t\t_, isString := value.(string)\n\t\treturn isString\n\t}',
            ).replace(
                "typeof value==='bigint'&&value>=1n&&value<=2n",
                "typeof value==='string'||(typeof value==='bigint'&&value>=1n&&value<=2n)",
            ).replace(
                'value.as_i64().is_some_and(|n|(1..=2).contains(&n))',
                'value.is_string()||value.as_i64().is_some_and(|n|(1..=2).contains(&n))',
            )
            assert accepts_string != actual
            production.write_text(accepts_string)
            run(f'{target}-wrong-primitive-validator-mutation', test, directory, expected=1)
            assert history.read_bytes() == saved
            assert receipt.read_text() == 'closed'
        report['source_hash_after'] = validate.digest(validate.validation_sources())
        if before != report['source_hash_after']:
            raise RuntimeError('Acceptance sources changed during the run')
        report['status'] = 'complete'
    except Exception as error:
        report['error'] = str(error)
    finally:
        history = ROOT / 'benchmarks/history/generators'
        history.mkdir(parents=True, exist_ok=True)
        with locked(history / '.lock'):
            with tempfile.NamedTemporaryFile(mode='w', dir=history, delete=False) as staged:
                json.dump(history_retention.mark(report), staged, indent=2)
                staged.write('\n'); staged.flush(); os.fsync(staged.fileno())
            destination = history / f'{stamp}-{os.getpid()}.json'
            os.replace(staged.name, destination)
            history_retention.prune(history)
        print(json.dumps({'status': report['status'], 'report': str(destination), 'error': report.get('error')}))
    return 0 if report['status'] == 'complete' else 1


if __name__ == '__main__':
    sys.exit(main())
