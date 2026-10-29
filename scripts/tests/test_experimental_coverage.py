"""Collector process isolation and real LLVM collection tests."""
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import signal
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('coverage_collect', ROOT / 'crates/dctx0-engine/src/coverage_collect.py')
collector = importlib.util.module_from_spec(spec)
spec.loader.exec_module(collector)


class ExperimentalCoverageTests(unittest.TestCase):
    def test_timeout_and_failure_are_explicit_and_reaped(self):
        _, _, code, timed_out, elapsed, rss = collector.run(
            [sys.executable, '-c', 'import time; time.sleep(30)'], ROOT, dict(os.environ), .1, True)
        self.assertTrue(timed_out)
        self.assertNotEqual(code, 0)
        self.assertLess(elapsed, 3000)
        with self.assertRaises(RuntimeError):
            collector.run([sys.executable, '-c', 'raise SystemExit(2)'], ROOT, dict(os.environ), 5)
        with self.assertRaises(FileNotFoundError):
            collector.run(['/nonexistent/dctx0-tool'], ROOT, dict(os.environ), 5)

    def test_real_llvm_profiles_are_separate_and_failed_tests_are_retained(self):
        if not shutil.which('rustc'):
            self.skipTest('Rust unavailable')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'src').mkdir()
            (root / 'Cargo.toml').write_text('[package]\nname="coverage-probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
            (root / 'src/lib.rs').write_text('''pub fn left() -> u32 { 1 }
pub fn right() -> u32 { 2 }
#[test] fn first() { assert_eq!(left(),1); }
#[test] fn second() { assert_eq!(right(),3); }
#[test] #[ignore] fn ignored() { panic!("must not run"); }
''')
            subprocess.run(['cargo', 'generate-lockfile'], cwd=root, check=True, capture_output=True)
            report = collector.collect({'root':str(root), 'artifacts':str(root / '.dctx0ctl/probe'), 'timeout_secs':30})
            executions = report['executions']
            self.assertEqual(len(executions),2)
            self.assertEqual({e['outcome'] for e in executions},{'pass','fail'})
            by_name = {e['test_id'].split('::')[-1]:e for e in executions}
            def count(execution, function):
                return sum(f['count'] for unit in execution['llvm_export']['data'] for f in unit['functions'] if function in f['name'])
            self.assertGreater(count(by_name['first'],'left'),0)
            self.assertEqual(count(by_name['first'],'right'),0)
            self.assertGreater(count(by_name['second'],'right'),0)
            self.assertEqual(count(by_name['second'],'left'),0)
            for execution in executions:
                self.assertTrue(execution['provenance']['coverage_available'])
                self.assertTrue(execution['provenance']['profile_base64'])
                self.assertGreater(execution['duration_ms'],0)
                self.assertGreater(execution['peak_rss_bytes'],0)
                self.assertTrue(Path(execution['provenance']['execution_artifacts'],'llvm.json').is_file())

    def test_interrupted_collection_reuses_atomic_completed_execution_files(self):
        if not shutil.which('rustc'):
            self.skipTest('Rust unavailable')
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'src').mkdir()
            (root / 'Cargo.toml').write_text('[package]\nname="resume-probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n')
            (root / 'src/lib.rs').write_text("""#[test] fn a_first(){
let marker=std::env::var("RESEARCH_MARKER").unwrap();
use std::io::Write;
let mut f=std::fs::OpenOptions::new().append(true).create(true).open(marker).unwrap();
f.write_all(b"one\\n").unwrap();
}
#[test] fn z_wait(){while !std::path::Path::new(&std::env::var("RESEARCH_RELEASE").unwrap()).exists(){std::thread::sleep(std::time::Duration::from_millis(20));}}
""")
            subprocess.run(['cargo', 'generate-lockfile', '--offline'], cwd=root, check=True, capture_output=True)
            artifacts = root / '.dctx0ctl/resume'
            env = {**os.environ, 'RESEARCH_MARKER': str(root / '.dctx0ctl/marker'),
                   'RESEARCH_RELEASE': str(root / '.dctx0ctl/release')}
            options = {'root': str(root), 'artifacts': str(artifacts), 'timeout_secs': 30,
                       'research': True, 'stream': True, 'resume': True}
            child = subprocess.Popen([sys.executable, str(ROOT / 'crates/dctx0-engine/src/coverage_collect.py'), json.dumps(options)], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                deadline = time.monotonic() + 30
                while not list(artifacts.glob('execution-*/execution.json')) and time.monotonic() < deadline:
                    time.sleep(.02)
                files = list(artifacts.glob('execution-*/execution.json'))
                self.assertEqual(len(files), 1)
                original = files[0].read_bytes()
                child.send_signal(signal.SIGTERM)
                child.communicate(timeout=5)
                self.assertNotEqual(child.returncode, 0)
                Path(env['RESEARCH_RELEASE']).touch()
                result = subprocess.run([sys.executable, str(ROOT / 'crates/dctx0-engine/src/coverage_collect.py'), json.dumps(options)], env=env, capture_output=True, timeout=30)
                self.assertEqual(result.returncode, 0, result.stderr.decode())
                self.assertEqual(len(json.loads(result.stdout)['execution_files']), 2)
                self.assertEqual(files[0].read_bytes(), original)
                self.assertEqual(Path(env['RESEARCH_MARKER']).read_text(), 'one\n')
            finally:
                if child.poll() is None:
                    child.kill()
                    child.communicate()



class CoverageDiffTests(unittest.TestCase):
    def test_git_ranges_old_coordinates_staging_paths_and_project_scope(self):
        namespace = dict(vars(collector))
        exec((ROOT / 'crates/dctx0-engine/src/coverage_diff.py').read_text(), namespace)
        diff = namespace['coverage_diff']
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            project = root / 'project with spaces'
            project.mkdir()
            def git(*args):
                return subprocess.run(['git', '-c', 'user.name=Coverage test', '-c', 'user.email=coverage@example.invalid', '-c', 'commit.gpgsign=false', '-c', 'core.hooksPath=/dev/null', *args], cwd=root, capture_output=True, check=True).stdout
            git('init', '-q')
            source = project / '[code].rs'
            source.write_text('first\nsecond\nthird\n')
            (project / 'binary.dat').write_bytes(b'\0old')
            (project / 'deleted.rs').write_text('old\n')
            (root / 'outside.rs').write_text('old\n')
            git('add', '.'); git('commit', '-qm', 'baseline')
            source.write_text('first\nCHANGED\nthird\n')
            (project / 'binary.dat').write_bytes(b'\0new')
            (project / 'deleted.rs').unlink()
            (root / 'outside.rs').write_text('new\n')
            git('add', '.')
            (project / 'new file.rs').write_text('new\n')
            report = diff({'root':str(project),'base':'HEAD'})
            files = {f['path']:f for f in report['files']}
            self.assertNotIn('outside.rs',files)
            self.assertEqual(files['[code].rs']['ranges'],[{'old_start':2,'old_count':1,'new_start':2,'new_count':1}])
            self.assertTrue(files['binary.dat']['binary'])
            self.assertEqual(files['deleted.rs']['status'],'D')
            self.assertEqual(files['new file.rs']['status'],'untracked')
            with self.assertRaises(RuntimeError):
                diff({'root':str(project),'base':'--help'})
            source.write_text('inserted\nfirst\nsecond\nthird\n')
            report = diff({'root':str(project),'base':'HEAD'})
            changed = next(f for f in report['files'] if f['path']=='[code].rs')
            self.assertEqual(changed['ranges'][0]['old_count'],0)


if __name__ == '__main__':
    unittest.main()
