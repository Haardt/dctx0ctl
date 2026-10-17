"""Read canonical verification artifact bytes from legacy and compressed histories."""
import gzip
import hashlib
import io
import sqlite3
from contextlib import closing


def saved_verification_artifacts(path, run_id):
    with closing(sqlite3.connect(f'file:{path}?mode=ro', uri=True)) as database:
        version = database.execute('PRAGMA user_version').fetchone()[0]
        columns = 'a.encoding,a.raw_length' if version >= 5 else "'identity',NULL"
        query = f'SELECT a.media_type,a.bytes,a.hash,{columns} FROM verification_artifacts a JOIN verification_run_artifacts r ON r.artifact_hash=a.hash WHERE r.run_id=?'
        for media, contents, expected, encoding, length in database.execute(query, (run_id,)):
            if encoding == 'gzip':
                if length is None or length < 0:
                    raise ValueError('Compressed artifact lacks its original size')
                with gzip.GzipFile(fileobj=io.BytesIO(contents)) as stream:
                    contents = stream.read(length + 1)
            elif encoding != 'identity':
                raise ValueError('Unsupported artifact storage encoding')
            if length is not None and len(contents) != length:
                raise ValueError('Artifact size mismatch')
            if hashlib.sha256(contents).hexdigest() != expected:
                raise ValueError('Artifact digest mismatch')
            yield media, contents, expected
