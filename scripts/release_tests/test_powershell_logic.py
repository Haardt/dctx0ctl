"""Optional Unix PowerShell control-flow fixture, explicitly not Windows ABI evidence."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PWSH = os.environ.get('DCTX0_PWSH') or shutil.which('pwsh')


@unittest.skipUnless(os.name != 'nt' and PWSH, 'Unix PowerShell logic fixture needs pwsh; Windows has native acceptance')
class PowerShellLogicTests(unittest.TestCase):
    def test_real_powershell_installer_control_flow_with_a_unix_execution_fixture(self):
        binary = Path(os.environ['DCTX0_RELEASE_BINARY']).resolve()
        import sys
        sys.path.insert(0, str(ROOT / 'scripts'))
        from release import workspace_version
        tag = 'v' + workspace_version()
        with tempfile.TemporaryDirectory(dir=ROOT / 'target') as temporary:
            directory = Path(temporary)
            # Production installer is untouched. Only the fixture removes the OS/architecture
            # gates and enables Unix executable permission; every installation assertion still runs.
            source = (ROOT / 'install.ps1').read_text()
            lines = [line for line in source.splitlines() if 'if ([Environment]::OSVersion.Platform' not in line and "if ($architecture -ne 'AMD64')" not in line]
            fixture = directory / 'unix-control-flow.ps1'
            fixture.write_text('\n'.join(lines).replace('        $installedVersion =', '        & chmod 755 $staged\n        $installedVersion =') + '\n')
            asset = directory / f'dctx0-{tag}-x86_64-pc-windows-msvc.zip'
            with zipfile.ZipFile(asset, 'w') as archive:
                archive.writestr('dctx0.exe', binary.read_bytes())
            result = subprocess.run([PWSH, '-NoProfile', '-File', str(ROOT / 'scripts/release_tests/test_windows_install.ps1'),
                                     '-Binary', str(binary), '-Assets', str(directory), '-Tag', tag, '-Installer', str(fixture),
                                     '-FixtureScope', 'Unix PowerShell control-flow fixture; not Windows OS or PE execution evidence'],
                                    capture_output=True, text=True, timeout=90)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn('PASS', result.stdout)
            self.assertIn('duplicate executable', result.stdout)
