"""Release and installer acceptance against a real CLI with deterministic network fixtures."""
import hashlib
import io
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
import release


class VersionAndArchiveTests(unittest.TestCase):
    def test_release_tags_match_the_workspace_and_distinguish_prereleases(self):
        version = release.workspace_version()
        self.assertEqual(release.validate_tag('v' + version)['version'], version)
        self.assertTrue(release.validate_tag('v1.2.3-rc.1', '1.2.3-rc.1')['prerelease'])
        self.assertFalse(release.validate_tag('v1.2.3+build.1', '1.2.3+build.1')['prerelease'])
        for tag in ['1.2.3', 'v01.2.3', 'v1.2', 'v1.2.3-01', 'v1.2.3/', 'v1.2.3\n', 'v1.2.3;echo injected']:
            with self.subTest(tag=tag), self.assertRaises(ValueError):
                release.validate_tag(tag, tag.removeprefix('v'))
        with self.assertRaises(ValueError):
            release.validate_tag('v99.99.99')

    def test_archives_are_reproducible_and_bind_the_architecture_and_binary(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for target, (kind, machine) in release.TARGETS.items():
                binary = root / 'binary'
                data = bytearray(128)
                if kind == 'elf':
                    data[:6] = b'\x7fELF\x02\x01'
                    data[18:20] = machine.to_bytes(2, 'little')
                elif kind == 'macho':
                    data[:4] = b'\xcf\xfa\xed\xfe'
                    data[4:8] = machine.to_bytes(4, 'little')
                else:
                    data[:2] = b'MZ'
                    data[60:64] = (64).to_bytes(4, 'little')
                    data[64:68] = b'PE\x00\x00'
                    data[68:70] = machine.to_bytes(2, 'little')
                    data[88:90] = b'\x0b\x02'
                binary.write_bytes(data)
                archive = release.package(binary, target, 'v' + release.workspace_version(), root, 42)
                first = archive.read_bytes()
                release.package(binary, target, 'v' + release.workspace_version(), root, 42)
                self.assertEqual(first, archive.read_bytes())
                if kind == 'pe':
                    with zipfile.ZipFile(archive) as bundle:
                        self.assertEqual(bundle.namelist(), ['dctx0.exe', 'LICENSE', 'release.json', 'README.txt'])
                        receipt = json.loads(bundle.read('release.json'))
                else:
                    with tarfile.open(archive) as bundle:
                        self.assertEqual(bundle.getnames(), ['dctx0', 'LICENSE', 'release.json', 'README.txt'])
                        self.assertEqual(bundle.getmember('dctx0').mode, 0o755)
                        receipt = json.load(bundle.extractfile('release.json'))
                self.assertEqual(receipt['target'], target)
                self.assertEqual(receipt['binary_sha256'], hashlib.sha256(data).hexdigest())
                other_target = next(name for name in release.TARGETS if name != target)
                with self.assertRaises(ValueError):
                    release.package(binary, other_target, 'v' + release.workspace_version(), root, 42)
            binary.write_text('not ELF')
            with self.assertRaises(ValueError):
                release.package(binary, target, 'v' + release.workspace_version(), root)

    def test_checksums_require_the_complete_exact_asset_set(self):
        tag = 'v' + release.workspace_version()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / 'install.sh').write_text('installer')
            (root / 'install.ps1').write_text('installer')
            with self.assertRaises(ValueError):
                release.checksums(root, tag)
            for target in release.TARGETS:
                (root / release.asset_name(tag, target)).write_bytes(target.encode())
            manifest = release.checksums(root, tag)
            lines = manifest.read_text().splitlines()
            self.assertEqual(len(lines), 7)
            for line in lines:
                digest, name = line.split('  ')
                self.assertEqual(digest, hashlib.sha256((root / name).read_bytes()).hexdigest())
            (root / 'unexpected').write_text('extra')
            with self.assertRaises(ValueError):
                release.checksums(root, tag)


@unittest.skipIf(platform.system() == 'Windows', 'PowerShell installer has its own native acceptance suite')
class InstallerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.binary = Path(os.environ['DCTX0_RELEASE_BINARY']).resolve()
        cls.version = release.workspace_version()
        cls.tag = 'v' + cls.version
        cls.target = release.native_target()
        cls.owner = tempfile.TemporaryDirectory(prefix='installer-acceptance-', dir=ROOT / 'target')
        cls.root = Path(cls.owner.name)
        cls.assets = cls.root / 'assets'
        cls.archive = release.package(cls.binary, cls.target, cls.tag, cls.assets)
        cls.shims = cls.root / 'shims'
        cls.shims.mkdir()
        (cls.shims / 'curl').write_text('''#!/usr/bin/env python3
import os,pathlib,shutil,sys
args=sys.argv[1:];url=args[-1];mode=os.environ.get('DCTX0_TEST_MODE','')
pathlib.Path(os.environ['DCTX0_TEST_CURL_LOG']).open('a').write(url+'\\n')
if mode=='download-failure':sys.exit(22)
if url.endswith('/latest'):
 print('https://github.com/'+os.environ.get('DCTX0_REPOSITORY','Haardt/dctx0ctl')+'/releases/tag/'+os.environ['DCTX0_TEST_TAG'] if mode!='bad-redirect' else 'https://example.com/tag/v0.1.0',end='');sys.exit(0)
source=pathlib.Path(os.environ['DCTX0_TEST_ASSETS'])/url.rsplit('/',1)[1]
if not source.is_file():sys.exit(22)
shutil.copyfile(source,args[args.index('--output')+1])
''')
        (cls.shims / 'uname').write_text('''#!/usr/bin/env bash
if [[ "$1" == -s ]]; then echo "${DCTX0_TEST_OS:-Linux}"; else echo "${DCTX0_TEST_ARCH}"; fi
''')
        (cls.shims / 'getconf').write_text('''#!/usr/bin/env bash
if [[ "${DCTX0_TEST_GLIBC:-glibc 2.35}" == unavailable ]]; then exit 1; fi
printf '%s\\n' "${DCTX0_TEST_GLIBC:-glibc 2.35}"
''')
        (cls.shims / 'sw_vers').write_text('#!/usr/bin/env bash\nprintf \'%s\\n\' \"${DCTX0_TEST_MACOS:-14.0}\"\n')
        for path in cls.shims.iterdir():
            path.chmod(0o755)

    @classmethod
    def tearDownClass(cls):
        cls.owner.cleanup()

    def setUp(self):
        self.owner = tempfile.TemporaryDirectory(dir=self.root)
        self.directory = Path(self.owner.name)
        self.assets = self.directory / 'assets'
        self.assets.mkdir()
        shutil.copyfile(self.archive, self.assets / self.archive.name)
        self.write_checksums()
        self.destination = self.directory / 'bin with spaces'
        self.destination.mkdir()
        self.env = {**os.environ, 'PATH': str(self.shims) + os.pathsep + os.environ['PATH'],
                    'DCTX0_TEST_ASSETS': str(self.assets), 'DCTX0_TEST_TAG': self.tag,
                    'DCTX0_TEST_ARCH': platform.machine(), 'DCTX0_TEST_OS': platform.system(), 'DCTX0_INSTALL_DIR': str(self.destination),
                    'DCTX0_TEST_CURL_LOG': str(self.directory / 'curl.log')}
        self.env.pop('DCTX0_REPOSITORY', None)
        self.env.pop('DCTX0_TEST_MODE', None)

    def tearDown(self):
        self.owner.cleanup()

    def write_checksums(self, content=None):
        content = content if content is not None else f'{hashlib.sha256((self.assets / self.archive.name).read_bytes()).hexdigest()}  {self.archive.name}\n'
        (self.assets / 'SHA256SUMS').write_text(content)

    def install(self, *args, text=None):
        return subprocess.run(['bash', '-s', '--', *args], input=text if text is not None else (ROOT / 'install.sh').read_text(),
                              env=self.env, cwd=self.directory, text=True, capture_output=True, timeout=30)

    def assert_unchanged_failure(self, *args, text=None):
        old = self.destination / 'dctx0'
        old.write_text('preserve this installation')
        result = self.install(*args, text=text)
        self.assertNotEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertEqual(old.read_text(), 'preserve this installation')
        self.assertEqual(list(self.destination.glob('.dctx0-install.*')), [])
        return result

    def test_piped_latest_install_works_without_checkout_assets(self):
        result = self.install()
        self.assertEqual(result.returncode, 0, result.stderr)
        installed = self.destination / 'dctx0'
        version = subprocess.check_output([installed, '--version'], cwd=self.directory, text=True).strip()
        self.assertEqual(version, 'dctx0 ' + self.version)
        skills = json.loads(subprocess.check_output([installed, 'skills', 'list', '--json'], cwd=self.directory))
        self.assertIn('dctx0-language', str(skills))
        specification = self.directory / 'spec'
        specification.mkdir()
        (specification / 'a.dctx0').write_text('workspace Installed { domain D { value Year = Int { range 1888..2100 } } }')
        result = subprocess.run([installed, 'check', specification, '--json'], cwd=self.directory, capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertTrue(json.loads(result.stdout)['ok'])

    def test_pinned_version_skips_latest_resolution_and_upgrades_atomically(self):
        (self.destination / 'dctx0').write_text('old regular binary')
        result = self.install('--version', self.version, '--bin-dir', str(self.destination))
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(hashlib.sha256((self.destination / 'dctx0').read_bytes()).hexdigest(),
                         hashlib.sha256(self.binary.read_bytes()).hexdigest())
        self.assertNotIn('/latest', (self.directory / 'curl.log').read_text())
        self.assertEqual(self.install('--version', self.tag).returncode, 0)

    def test_bad_checksums_and_duplicates_preserve_the_existing_installation(self):
        for manifest in ['0' * 64 + '  ' + self.archive.name + '\n', 'no valid entry\n',
                         ('0' * 64 + '  ' + self.archive.name + '\n') * 2]:
            with self.subTest(manifest=manifest):
                self.write_checksums(manifest)
                self.assert_unchanged_failure('--version', self.tag)

    def test_failed_download_and_unexpected_redirect_preserve_the_installation(self):
        for mode in ['download-failure', 'bad-redirect']:
            with self.subTest(mode=mode):
                self.env['DCTX0_TEST_MODE'] = mode
                self.assert_unchanged_failure()

    def test_unsupported_platform_architecture_and_libc_do_not_fetch_assets(self):
        for key, value in [('DCTX0_TEST_OS', 'FreeBSD'), ('DCTX0_TEST_ARCH', 'riscv64')]:
            with self.subTest(key=key, value=value):
                previous = self.env.get(key)
                self.env[key] = value
                self.assert_unchanged_failure()
                if previous is None:
                    del self.env[key]
                else:
                    self.env[key] = previous
                self.assertFalse((self.directory / 'curl.log').exists())

    def test_platform_minimum_versions_are_checked_before_download(self):
        for system, key, value in [('Linux', 'DCTX0_TEST_GLIBC', 'glibc 2.34'), ('Linux', 'DCTX0_TEST_GLIBC', 'unavailable'), ('Darwin', 'DCTX0_TEST_MACOS', '13.7')]:
            with self.subTest(system=system, value=value):
                self.env['DCTX0_TEST_OS'] = system
                self.env[key] = value
                self.assert_unchanged_failure()
                self.env.pop(key)
                self.assertFalse((self.directory / 'curl.log').exists())

    def test_symlinks_and_non_regular_destinations_are_not_replaced(self):
        target = self.directory / 'keep'
        target.write_text('outside installation')
        link = self.destination / 'dctx0'
        link.symlink_to(target)
        self.assertNotEqual(self.install().returncode, 0)
        self.assertEqual(target.read_text(), 'outside installation')
        link.unlink()
        link.mkdir()
        self.assertNotEqual(self.install().returncode, 0)
        self.assertTrue(link.is_dir())

    def test_version_mismatch_and_missing_binary_member_preserve_the_installation(self):
        for name in ['dctx0', 'other']:
            with self.subTest(name=name):
                payload = b'#!/usr/bin/env bash\necho dctx0 99.99.99\n'
                with tarfile.open(self.assets / self.archive.name, 'w:gz') as bundle:
                    member = tarfile.TarInfo(name)
                    member.size = len(payload)
                    bundle.addfile(member, io.BytesIO(payload))
                self.write_checksums()
                self.assert_unchanged_failure('--version', self.tag)

    def test_archive_paths_are_never_unpacked(self):
        with tarfile.open(self.assets / self.archive.name, 'w:gz') as bundle:
            data = self.binary.read_bytes()
            member = tarfile.TarInfo('dctx0')
            member.size = len(data)
            bundle.addfile(member, io.BytesIO(data))
            member = tarfile.TarInfo('../../unexpected-file')
            member.size = 1
            bundle.addfile(member, io.BytesIO(b'x'))
        self.write_checksums()
        result = self.install('--version', self.tag)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse((self.directory / 'unexpected-file').exists())
        self.assertFalse((self.root / 'unexpected-file').exists())

    def test_invalid_arguments_and_truncated_script_do_not_install(self):
        for args in [('--version',), ('--version', 'v0.1.0;echo unsafe'), ('--bin-dir',), ('--unknown',)]:
            with self.subTest(args=args):
                self.assert_unchanged_failure(*args)
        self.assert_unchanged_failure(text=(ROOT / 'install.sh').read_text()[:-2])
        self.assertFalse((self.directory / 'curl.log').exists())
        self.assertEqual(self.install('--help').returncode, 0)


if __name__ == '__main__':
    unittest.main()
