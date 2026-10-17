import gzip
import hashlib
from pathlib import Path
import sqlite3
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from verification_storage import saved_verification_artifacts


class ArtifactStorageTests(unittest.TestCase):
    def test_legacy_and_compressed_content_keep_the_original_digest(self):
        for version, encoding in [(4, 'identity'), (5, 'identity'), (5, 'gzip')]:
            with self.subTest(version=version, encoding=encoding), tempfile.TemporaryDirectory() as temporary:
                path = Path(temporary) / 'state.sqlite3'
                original = b'{"metric":42}' * 1000
                digest = hashlib.sha256(original).hexdigest()
                with sqlite3.connect(path) as database:
                    extra = ',encoding TEXT,raw_length INTEGER' if version == 5 else ''
                    database.execute(f'CREATE TABLE verification_artifacts(media_type TEXT,bytes BLOB,hash TEXT{extra})')
                    database.execute('CREATE TABLE verification_run_artifacts(run_id TEXT,artifact_hash TEXT)')
                    database.execute(f'PRAGMA user_version={version}')
                    row = ['application/json', gzip.compress(original) if encoding == 'gzip' else original, digest]
                    if version == 5:
                        row += [encoding, len(original)]
                    database.execute('INSERT INTO verification_artifacts VALUES(' + ','.join('?' for _ in row) + ')', row)
                    database.execute('INSERT INTO verification_run_artifacts VALUES(?,?)', ('run', digest))
                self.assertEqual(list(saved_verification_artifacts(path, 'run')), [('application/json', original, digest)])
                with sqlite3.connect(path) as database:
                    database.execute('UPDATE verification_artifacts SET bytes=?', (b'corrupt',))
                with self.assertRaises((ValueError, OSError)):
                    list(saved_verification_artifacts(path, 'run'))


if __name__ == '__main__':
    unittest.main()
