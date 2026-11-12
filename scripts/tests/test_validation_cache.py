import contextlib
import os
from pathlib import Path
import sqlite3
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import validation_cache as cache


class ValidationCacheTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        subprocess.run(['git', 'init', '-q', str(self.root)], check=True)
        (self.root / 'source').write_text('one')
        subprocess.run(['git', 'add', 'source'], cwd=self.root, check=True)
        self.evidence = self.root / 'target' / 'report.json'
        self.evidence.parent.mkdir()
        self.evidence.write_text('pass')
        self.tool_patch = mock.patch.object(cache, '_tool', side_effect=lambda name, executable=None: {'name': name})
        self.tool_patch.start()
        self.addCleanup(self.tool_patch.stop)

    def ident(self, options=None):
        return cache.identity(self.root, options or {'coverage': True})

    def test_receipt_requires_unchanged_inputs_options_and_artifact(self):
        identity = self.ident()
        receipt = cache.record_success(self.root, identity, [self.evidence])
        self.assertEqual(cache.find_receipt(self.root, identity)['id'], receipt['id'])
        (self.root / 'source').write_text('two')
        self.assertIsNone(cache.find_receipt(self.root, self.ident()))
        (self.root / 'source').write_text('one')
        self.assertIsNone(cache.find_receipt(self.root, self.ident({'coverage': False})))
        self.evidence.write_text('changed')
        self.assertIsNone(cache.find_receipt(self.root, identity))

    def test_untracked_deleted_and_symlink_inputs_are_hashed(self):
        initial = self.ident()
        (self.root / 'new').write_text('test')
        added = self.ident()
        self.assertNotEqual(initial, added)
        (self.root / 'new').unlink()
        (self.root / 'source').unlink()
        self.assertNotEqual(initial, self.ident())
        (self.root / 'new').write_text('linked')
        (self.root / 'source').symlink_to('new')
        linked = self.ident()
        self.assertNotEqual(initial, linked)
        (self.root / 'new').write_text('changed linked content')
        self.assertNotEqual(linked, self.ident())

    def test_external_symlink_target_changes_invalidate_receipt(self):
        with tempfile.TemporaryDirectory() as external:
            target = Path(external) / 'external'
            target.write_text('first')
            (self.root / 'link').symlink_to(target)
            ident = self.ident()
            cache.record_success(self.root, ident, [self.evidence])
            self.assertIsNotNone(cache.find_receipt(self.root, self.ident()))
            target.write_text('second')
            self.assertIsNone(cache.find_receipt(self.root, self.ident()))
            (self.root / 'link').unlink()
            (self.root / 'link').symlink_to(external, target_is_directory=True)
            with self.assertRaises(ValueError):
                self.ident()

    def test_external_cargo_configuration_changes_invalidate_receipt(self):
        with tempfile.TemporaryDirectory() as cargo_home:
            with mock.patch.dict(os.environ, {'CARGO_HOME': cargo_home}):
                ident = self.ident()
                cache.record_success(self.root, ident, [self.evidence])
                self.assertIsNotNone(cache.find_receipt(self.root, self.ident()))
                (Path(cargo_home) / 'config.toml').write_text('[build]\njobs = 2\n')
                self.assertIsNone(cache.find_receipt(self.root, self.ident()))

    def test_real_tool_identity_receipt_and_input_invalidation(self):
        self.tool_patch.stop()
        identity = self.ident()
        cache.record_success(self.root, identity, [self.evidence])
        self.assertIsNotNone(cache.find_receipt(self.root, self.ident()))
        (self.root / 'source').write_text('changed')
        self.assertIsNone(cache.find_receipt(self.root, self.ident()))
        (self.root / 'source').write_text('one')
        (self.root / 'new-input').write_text('added')
        self.assertIsNone(cache.find_receipt(self.root, self.ident()))
        (self.root / 'new-input').unlink()
        (self.root / 'source').unlink()
        self.assertIsNone(cache.find_receipt(self.root, self.ident()))
        self.tool_patch.start()

    def test_tool_identity_changes_invalidate_receipt(self):
        ident = self.ident()
        cache.record_success(self.root, ident, [self.evidence])
        with mock.patch.object(cache, '_tool', return_value={'version': 'changed'}):
            self.assertIsNone(cache.find_receipt(self.root, self.ident()))

    def test_receipts_are_append_only_and_age_bounded(self):
        identity = self.ident()
        first = cache.record_success(self.root, identity, [self.evidence])
        second = cache.record_success(self.root, identity, [self.evidence])
        self.assertGreater(second['id'], first['id'])
        self.assertIsNone(cache.find_receipt(self.root, identity, max_age_seconds=0))
        with contextlib.closing(cache._connect(self.root)) as db:
            with self.assertRaises(sqlite3.IntegrityError):
                db.execute('DELETE FROM receipts')

    def test_no_evidence_and_symlink_evidence_are_rejected(self):
        with self.assertRaises(ValueError):
            cache.record_success(self.root, self.ident(), [])
        link = self.root / 'target' / 'link'
        link.symlink_to(self.evidence)
        with self.assertRaises(ValueError):
            cache.record_success(self.root, self.ident(), [link])

    def test_environment_changes_invalidate(self):
        identity = self.ident()
        with mock.patch.dict(os.environ, {'DCTX0_VALIDATION_ARTIFACT_ROOT': '/output/new-run',
                                         'DCTX0_MEMORY_BUDGET_BYTES': '123'}):
            self.assertEqual(identity, self.ident())
        with mock.patch.dict(os.environ, {'CACHE_TEST_VARIABLE': 'changed'}):
            self.assertNotEqual(identity, self.ident())


if __name__ == '__main__':
    unittest.main()
