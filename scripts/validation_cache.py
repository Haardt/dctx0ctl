"""Local historical validation receipts; never treat a receipt as a new run.

The identity covers repository inputs, selected tool binaries and the process
 environment. Network services, external files and other dynamic system state
are not hermetically captured. Callers must support bypassing cached receipts.
"""
import contextlib
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import sqlite3
import stat
import subprocess
import sys

VERSION = 1
TOOLS = ('cargo', 'rustc', 'rustup', 'go', 'bun', 'cargo-llvm-cov', 'llvm-cov', 'llvm-profdata', 'cc', 'c++', 'git', 'ld')


def _json(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True)


def _hash_bytes(data):
    return hashlib.sha256(data).hexdigest()


def _file_hash(path):
    digest = hashlib.sha256()
    with open(path, 'rb') as handle:
        for block in iter(lambda: handle.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def _excluded(path):
    parts = Path(os.fsdecode(path)).parts
    return 'target' in parts or parts[:2] == ('benchmarks', 'history') or '__pycache__' in parts


def _inventory(root):
    # --cached retains deleted tracked paths. --others adds nonignored inputs.
    result = subprocess.run(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'],
                            cwd=root, check=True, stdout=subprocess.PIPE)
    entries = []
    for name in sorted(set(result.stdout.split(b'\0')) - {b''}):
        if _excluded(name):
            continue
        path = os.path.join(os.fsencode(root), name)
        try:
            info = os.lstat(path)
        except FileNotFoundError:
            entries.append([os.fsdecode(name), 'missing'])
            continue
        mode = stat.S_IMODE(info.st_mode)
        if stat.S_ISLNK(info.st_mode):
            resolved = Path(os.fsdecode(path)).resolve(strict=True)
            if not resolved.is_file():
                raise ValueError('Cannot safely inventory linked non-file input: ' + os.fsdecode(name))
            entries.append([os.fsdecode(name), 'symlink', mode, os.fsdecode(os.readlink(path)),
                            str(resolved), _file_hash(resolved)])
        elif stat.S_ISREG(info.st_mode):
            entries.append([os.fsdecode(name), 'file', mode, _file_hash(path)])
        else:
            raise ValueError('Unsupported repository input: ' + os.fsdecode(name))
    # Package managers usually ignore installed dependencies in Git. Inventory
    # their bytes as well; reject directory links whose contents are not bounded.
    for base, directories, files in os.walk(root, followlinks=False):
        relative = Path(base).relative_to(root)
        directories[:] = [d for d in directories if d not in ('.git', 'target', '__pycache__')
                          and relative / d != Path('benchmarks/history')]
        if 'node_modules' not in relative.parts:
            continue
        for name in sorted(directories + files):
            path = Path(base) / name
            rel = str(path.relative_to(root))
            if path.is_symlink():
                resolved = path.resolve(strict=True)
                if resolved.is_dir():
                    raise ValueError('Cannot safely inventory linked dependency directory: ' + rel)
                entries.append([rel, 'dependency-link', os.readlink(path), _file_hash(resolved)])
            elif path.is_file():
                entries.append([rel, 'dependency', stat.S_IMODE(path.stat().st_mode), _file_hash(path)])
    return entries


def _tool(name, executable=None):
    selected = executable or shutil.which(name)
    if selected is None:
        return {'name': name, 'available': False}
    path = Path(selected).resolve(strict=True)
    result = subprocess.run([str(selected), '--version'], stdout=subprocess.PIPE,
                            stderr=subprocess.STDOUT, timeout=30, check=False)
    return {'name': name, 'available': True, 'selected': os.path.abspath(selected),
            'resolved': str(path), 'sha256': _file_hash(path), 'version_exit': result.returncode,
            'version': result.stdout.decode('utf-8', errors='replace').strip(),
            'version_sha256': _hash_bytes(result.stdout)}


def _cargo_config(root):
    cargo_home = Path(os.environ.get('CARGO_HOME', Path.home() / '.cargo')).expanduser()
    if not cargo_home.is_absolute():
        cargo_home = root / cargo_home
    configs = []
    paths = [directory / '.cargo' / name for directory in (root, *root.parents)
             for name in ('config', 'config.toml')]
    paths += [cargo_home / name for name in ('config', 'config.toml')]
    for path in paths:
        if path.exists():
            if not path.is_file():
                raise ValueError('Cannot safely inventory Cargo configuration: ' + str(path))
            configs.append([str(path.absolute()), str(path.resolve(strict=True)), _file_hash(path)])
        else:
            configs.append([str(path.absolute()), 'missing'])
    return configs


def identity(root, options):
    """Capture relevant local inputs. Raises when inputs cannot be read reliably."""
    root = Path(root).resolve(strict=True)
    entries = _inventory(root)
    # Hash the environment rather than storing potentially sensitive values.
    # The retained artifact directory is newly allocated per validation. It is
    # an output location, not a test input; evidence paths are bound separately.
    environment = {key: value for key, value in os.environ.items()
                   if key not in ('DCTX0_MEMORY_BUDGET_BYTES', 'DCTX0_VALIDATION_ARTIFACT_ROOT')}
    environment_hash = _hash_bytes(_json(sorted(environment.items())).encode())
    tools = [_tool(name) for name in TOOLS]
    tools.append(_tool('python', sys.executable))
    # Rustup proxies remain byte-identical across toolchain switches. Capture
    # the selected tools and LLVM instrumentation binaries behind the proxy.
    for name in ('cargo', 'rustc'):
        if shutil.which('rustup'):
            selected = subprocess.run(['rustup', 'which', name], cwd=root, capture_output=True, text=True, check=False)
            if selected.returncode == 0:
                tools.append(_tool('rustup-' + name, selected.stdout.strip()))
    if shutil.which('rustc'):
        sysroot = subprocess.run(['rustc', '--print', 'sysroot'], cwd=root, capture_output=True, text=True, check=False)
        if sysroot.returncode == 0:
            for name in ('llvm-cov', 'llvm-profdata'):
                for path in sorted(Path(sysroot.stdout.strip()).glob('lib/rustlib/*/bin/' + name)):
                    tools.append(_tool('rust-toolchain-' + name, str(path)))
    data = {'version': VERSION, 'root': str(root), 'inputs_sha256': _hash_bytes(_json(entries).encode()),
            'inputs_count': len(entries), 'environment_sha256': environment_hash,
            'tools': tools, 'cargo_config': _cargo_config(root), 'platform': platform.platform(), 'machine': platform.machine(),
            'options': options}
    # Ensure options can be serialized before doing any validation work.
    return json.loads(_json(data))


def _artifacts(paths):
    artifacts = []
    for raw in paths:
        path = Path(raw)
        if not path.is_absolute() or path.is_symlink() or not path.is_file():
            raise ValueError('Receipt artifacts must be absolute, non-symlink regular files')
        artifacts.append({'path': str(path), 'sha256': _file_hash(path)})
    if not artifacts:
        raise ValueError('A successful receipt requires at least one evidence artifact')
    return artifacts


def _connect(root):
    directory = Path(root) / 'target' / 'validation-cache'
    directory.mkdir(mode=0o700, parents=True, exist_ok=True)
    db = sqlite3.connect(directory / 'receipts.sqlite3', timeout=30)
    db.execute('PRAGMA busy_timeout=30000')
    db.executescript('''
        CREATE TABLE IF NOT EXISTS receipts (
            id INTEGER PRIMARY KEY, created TEXT NOT NULL, identity TEXT NOT NULL,
            artifacts TEXT NOT NULL, version INTEGER NOT NULL
        );
        CREATE TRIGGER IF NOT EXISTS receipts_no_update BEFORE UPDATE ON receipts
        BEGIN SELECT RAISE(ABORT, 'immutable receipt'); END;
        CREATE TRIGGER IF NOT EXISTS receipts_no_delete BEFORE DELETE ON receipts
        BEGIN SELECT RAISE(ABORT, 'immutable receipt'); END;
    ''')
    return db


def record_success(root, identity_data, artifacts):
    """Append a receipt only after the caller establishes full success and unchanged identity."""
    if identity_data.get('version') != VERSION:
        raise ValueError('Unsupported receipt identity version')
    evidence = _artifacts(artifacts)
    created = datetime.datetime.now(datetime.timezone.utc).isoformat()
    with contextlib.closing(_connect(root)) as db, db:
        cursor = db.execute('INSERT INTO receipts(created, identity, artifacts, version) VALUES (?, ?, ?, ?)',
                            (created, _json(identity_data), _json(evidence), VERSION))
        receipt_id = cursor.lastrowid
    return {'id': receipt_id, 'created': created, 'identity': identity_data, 'artifacts': evidence}


def find_receipt(root, identity_data, max_age_seconds=None):
    """Find newest matching receipt whose evidence still exists and has identical bytes."""
    if max_age_seconds is not None and max_age_seconds < 0:
        raise ValueError('max_age_seconds must be nonnegative')
    with contextlib.closing(_connect(root)) as db:
        rows = db.execute('SELECT id, created, artifacts FROM receipts WHERE version = ? AND identity = ? ORDER BY id DESC',
                          (VERSION, _json(identity_data))).fetchall()
    now = datetime.datetime.now(datetime.timezone.utc)
    for receipt_id, created, encoded in rows:
        try:
            timestamp = datetime.datetime.fromisoformat(created)
            age = (now - timestamp).total_seconds()
            if age < 0 or (max_age_seconds is not None and age > max_age_seconds):
                continue
            evidence = json.loads(encoded)
            if not evidence or _artifacts([item['path'] for item in evidence]) != evidence:
                continue
        except (ValueError, OSError, KeyError, TypeError, json.JSONDecodeError):
            continue
        return {'id': receipt_id, 'created': created, 'identity': identity_data, 'artifacts': evidence}
    return None
