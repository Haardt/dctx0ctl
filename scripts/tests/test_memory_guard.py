import argparse
import importlib.util
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('memory_guard', Path(__file__).resolve().parents[1] / 'memory_guard.py')
guard = importlib.util.module_from_spec(spec)
spec.loader.exec_module(guard)


class MemoryGuardTests(unittest.TestCase):
    def test_budget_uses_one_third_available_memory_and_rejects_invalid_values(self):
        self.assertEqual(guard.memory_budget('MemFree: 1 kB\nMemAvailable: 3000 kB'), 1024000)
        for text in ('', 'MemAvailable: 0 kB', 'MemAvailable: -1 kB', 'MemAvailable: 3 MB'):
            with self.assertRaises(ValueError):
                guard.memory_budget(text)

    def test_controller_scope_and_watchdog_cap_descendants_and_preserve_exit_status(self):
        with patch.dict(guard.os.environ, {'DCTX0_MEMORY_GUARD': '1'}, clear=True), patch.object(guard.sys, 'platform', 'linux'), patch.object(guard, 'systemd_available', return_value=True), patch.object(guard.Path, 'read_text', return_value='MemAvailable: 3000 kB'), patch.object(guard, 'monitored_run', return_value=137) as run:
            self.assertEqual(guard.guarded_run(['bun', 'worker.ts']), 137)
        command, budget, environment = run.call_args.args
        self.assertIn('MemoryMax=1024000', command)
        self.assertIn('MemorySwapMax=0', command)
        self.assertEqual(command[-2:], ['bun', 'worker.ts'])
        self.assertEqual(budget, 1024000)
        self.assertEqual(environment['CARGO_BUILD_JOBS'], '1')
        self.assertEqual(environment['RUST_TEST_THREADS'], '1')
        with self.assertRaises(ValueError):
            guard.guarded_run([])

    def test_linux_without_a_controller_uses_watchdog_and_never_falls_back_unbounded(self):
        with patch.dict(guard.os.environ, {'DCTX0_MEMORY_GUARD': '1'}), patch.object(guard.sys, 'platform', 'linux'), patch.object(guard, 'systemd_available', return_value=False), patch.object(guard.Path, 'read_text', return_value='MemAvailable: 3000 kB'), patch.object(guard, 'monitored_run', return_value=7) as run, patch.object(guard.subprocess, 'run') as unbounded:
            self.assertEqual(guard.guarded_run(['worker']), 7)
            self.assertEqual(run.call_args.args[:2], (['worker'], 1024000))
            unbounded.assert_not_called()
            run.side_effect = OSError('monitor unavailable')
            with self.assertRaises(OSError):
                guard.guarded_run(['worker'])
            unbounded.assert_not_called()

    def test_explicitly_disabled_or_unsupported_platform_runs_once(self):
        for setting, platform in [('0', 'linux'), ('1', 'darwin')]:
            with patch.dict(guard.os.environ, {'DCTX0_MEMORY_GUARD': setting}), patch.object(guard.sys, 'platform', platform), patch.object(guard.Path, 'read_text') as read, patch.object(guard.subprocess, 'run', return_value=argparse.Namespace(returncode=7)) as run:
                self.assertEqual(guard.guarded_run(['worker']), 7)
                run.assert_called_once_with(['worker'])
                read.assert_not_called()

    def test_systemd_manager_without_memory_controller_is_not_enforcement(self):
        with patch.object(guard.sys, 'platform', 'linux'), patch.object(guard.shutil, 'which', return_value='/usr/bin/tool'), patch.object(guard.Path, 'read_text', return_value='cpu io'), patch.object(guard.subprocess, 'run') as run:
            self.assertFalse(guard.systemd_available())
            run.assert_not_called()
        with patch.object(guard.sys, 'platform', 'linux'), patch.object(guard.shutil, 'which', return_value='/usr/bin/tool'), patch.object(guard.Path, 'read_text', return_value='cpu memory'):
            for result in (0, 1):
                with patch.object(guard.subprocess, 'run', return_value=argparse.Namespace(returncode=result)):
                    self.assertEqual(guard.systemd_available(), result == 0)
            with patch.object(guard.subprocess, 'run', side_effect=guard.subprocess.TimeoutExpired('systemctl', 5)):
                self.assertFalse(guard.systemd_available())
        with patch.object(guard.sys, 'platform', 'darwin'):
            self.assertFalse(guard.systemd_available())
        with patch.object(guard.shutil, 'which', return_value=None):
            self.assertFalse(guard.systemd_available())

    def test_sample_keeps_detached_descendants_and_rejects_reused_pids(self):
        me = os.getpid()
        table = {10:(me,10,'a',20),11:(10,11,'b',30),12:(1,12,'old',40)}
        with patch.object(guard, 'processes', return_value=table):
            tracked, rss = guard.sample(10, {12:'old'})
            self.assertEqual(rss, 90)
        table[11]=(1,11,'b',30)
        table[12]=(1,12,'reused',400)
        with patch.object(guard, 'processes', return_value=table):
            tracked, rss = guard.sample(10, tracked)
            self.assertEqual(rss, 50)
            self.assertNotIn(12, tracked)

    @unittest.skipUnless(sys.platform == 'linux', 'Linux procfs watchdog')
    def test_real_aggregate_rss_abort_kills_and_reaps_detached_children(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory)/'child.pid'
            child = 'import os,time;os.setsid();data=bytearray(20*1024*1024);time.sleep(30)'
            parent = f'import subprocess,sys,time;data=bytearray(20*1024*1024);p=subprocess.Popen([sys.executable,"-c",{child!r}]);open({str(pidfile)!r},"w").write(str(p.pid));time.sleep(30)'
            helper = f'import runpy,os,sys;m=runpy.run_path({str(Path(guard.__file__).resolve())!r});sys.exit(m["monitored_run"]([sys.executable,"-c",{parent!r}],48*1024*1024,dict(os.environ)))'
            result = subprocess.run([sys.executable,'-c',helper],capture_output=True,text=True,timeout=10)
            self.assertEqual(result.returncode,137,result.stdout+result.stderr)
            self.assertIn('ABORT (RSS budget / system reserve)',result.stdout)
            self.assertIn('peak_process_tree_rss_bytes=',result.stdout)
            self.assertFalse((Path('/proc')/pidfile.read_text()).exists())

    @unittest.skipUnless(sys.platform == 'linux', 'Linux procfs watchdog')
    def test_real_success_preserves_status_and_cancellation_cleans_up(self):
        helper = f'import runpy,os,sys;m=runpy.run_path({str(Path(guard.__file__).resolve())!r});sys.exit(m["monitored_run"]([sys.executable,"-c","raise SystemExit(7)"],64*1024*1024,dict(os.environ)))'
        result=subprocess.run([sys.executable,'-c',helper],capture_output=True,text=True,timeout=10)
        self.assertEqual(result.returncode,7,result.stdout+result.stderr)
        with tempfile.TemporaryDirectory() as directory:
            pidfile=Path(directory)/'child.pid'
            payload=f'import os,time;open({str(pidfile)!r},"w").write(str(os.getpid()));time.sleep(30)'
            helper=f'import runpy,os,sys;m=runpy.run_path({str(Path(guard.__file__).resolve())!r});sys.exit(m["monitored_run"]([sys.executable,"-c",{payload!r}],64*1024*1024,dict(os.environ)))'
            child=subprocess.Popen([sys.executable,'-c',helper],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
            try:
                deadline=time.monotonic()+5
                while not pidfile.exists() and time.monotonic()<deadline:
                    time.sleep(0.01)
                self.assertTrue(pidfile.exists())
                child.send_signal(signal.SIGTERM)
                stdout,stderr=child.communicate(timeout=10)
                self.assertEqual(child.returncode,130,stdout+stderr)
                self.assertFalse((Path('/proc')/pidfile.read_text()).exists())
            finally:
                if child.poll() is None:
                    child.kill()
                    child.wait()

    @unittest.skipUnless(sys.platform == 'linux', 'Linux child subreaper')
    def test_finished_adopted_children_are_reaped_while_root_runs_without_stealing_its_status(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory) / 'adopted.pid'
            launcher = (
                'import os,time;from pathlib import Path;'
                'pid=os.fork();'
                'os._exit(0) if pid else None;'
                f'path=Path({str(pidfile)!r});'
                'path.with_suffix(".tmp").write_text(str(os.getpid()));'
                'os.replace(path.with_suffix(".tmp"),path);'
                'time.sleep(0.1)'
            )
            root = f'''
import os, subprocess, sys, time
from pathlib import Path
subprocess.run([sys.executable, '-c', {launcher!r}], check=True)
path = Path({str(pidfile)!r})
deadline = time.monotonic() + 3
while not path.exists() and time.monotonic() < deadline:
    time.sleep(0.01)
pid = int(path.read_text())
while time.monotonic() < deadline:
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        raise SystemExit(7)
    time.sleep(0.01)
raise SystemExit(33)
'''
            helper = f'import runpy,os,sys;m=runpy.run_path({str(Path(guard.__file__).resolve())!r});sys.exit(m["monitored_run"]([sys.executable,"-c",{root!r}],64*1024*1024,dict(os.environ)))'
            result = subprocess.run([sys.executable, '-c', helper], capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 7, result.stdout + result.stderr)
            self.assertFalse((Path('/proc') / pidfile.read_text()).exists())
