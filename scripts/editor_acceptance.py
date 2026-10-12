#!/usr/bin/env python3
"""Contributor QA for native editor distributions; product installation needs no Python."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import tempfile
import threading
import time
import xml.etree.ElementTree as ET
import zipfile

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / 'target/editors'

class Client:
    def __init__(self, binary, root):
        self.child = subprocess.Popen([str(binary), 'lsp', '--stdio', '--project', str(root)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        self.messages = queue.Queue()
        self.number = 0
        def read():
            try:
                while True:
                    length = None
                    while True:
                        header = self.child.stdout.readline()
                        if not header:
                            return
                        if header == b'\r\n':
                            break
                        if header.startswith(b'Content-Length:'):
                            length = int(header.split(b':', 1)[1])
                    assert length is not None and length <= 4 * 1024 * 1024
                    self.messages.put(json.loads(self.child.stdout.read(length)))
            except Exception as error:
                self.messages.put(error)
        self.reader = threading.Thread(target=read, daemon=True)
        self.reader.start()
    def send(self, value):
        body = json.dumps({'jsonrpc': '2.0', **value}).encode()
        self.child.stdin.write(f'Content-Length: {len(body)}\r\n\r\n'.encode() + body)
        self.child.stdin.flush()
    def request(self, method, params):
        self.number += 1
        number = self.number
        self.send({'id': number, 'method': method, 'params': params})
        deadline = time.monotonic() + 15
        while True:
            message = self.messages.get(timeout=max(0.001, deadline - time.monotonic()))
            if isinstance(message, Exception):
                raise message
            if message.get('method') == 'textDocument/publishDiagnostics':
                assert not message['params']['diagnostics'], message
            if message.get('id') == number:
                assert 'error' not in message, message
                return message['result']
    def notify(self, method, params):
        self.send({'method': method, 'params': params})
    def rss(self):
        if os.name != 'posix' or not Path('/proc').exists():
            return None
        status = Path(f'/proc/{self.child.pid}/status').read_text()
        return next(int(line.split()[1]) * 1024 for line in status.splitlines() if line.startswith('VmHWM:'))
    def close(self):
        try:
            self.request('shutdown', None)
            self.notify('exit', None)
            # Keep stdin open: exit must not depend on a subsequent client EOF.
            assert self.child.wait(timeout=3) == 0
        finally:
            if self.child.poll() is None:
                self.child.kill()
                self.child.wait()
            for stream in (self.child.stdin, self.child.stdout, self.child.stderr):
                stream.close()
            self.reader.join(timeout=1)

def run(command, **kwargs):
    subprocess.run(command, cwd=ROOT, check=True, timeout=120, **kwargs)

def functional(binary):
    tool = shutil.which('tree-sitter')
    if not tool:
        raise RuntimeError('Contributor acceptance requires tree-sitter CLI 0.26.13 in PATH; the product does not require it')
    grammar = ROOT / 'editors/tree-sitter-dctx0'
    subprocess.run([tool, 'test'], cwd=grammar, check=True, timeout=30)
    sources = sorted((ROOT / 'examples/cinema/dctx0').glob('*.dctx0'))
    subprocess.run([tool, 'parse', '--quiet', *map(str, sources)], cwd=grammar, check=True, timeout=30)
    for name in ('highlights', 'outline', 'brackets'):
        subprocess.run([tool, 'query', str(ROOT / f'editors/zed/languages/dctx0/{name}.scm'), *map(str, sources)], cwd=grammar, stdout=subprocess.DEVNULL, check=True, timeout=30)
    with tempfile.TemporaryDirectory(prefix='distribution-', dir=OUT) as temporary:
        destination = Path(temporary) / 'bundles'
        for editor in ('vscode', 'zed'):
            command = [str(binary), '--json', 'editor', 'install', editor, '--output', str(destination)]
            preview = json.loads(subprocess.check_output([*command, '--dry-run'], cwd=ROOT))
            assert preview['status'] == 'planned' and not destination.joinpath(editor).exists()
            report = json.loads(subprocess.check_output(command, cwd=ROOT))
            assert report['status'] == 'prepared'
            assert json.loads(subprocess.check_output(command, cwd=ROOT))['status'] == 'unchanged'
        vsix = destination / 'vscode/dctx0.vsix'
        with zipfile.ZipFile(vsix) as archive:
            ET.fromstring(archive.read('extension.vsixmanifest'))
            ET.fromstring(archive.read('[Content_Types].xml'))
            package = json.loads(archive.read('extension/package.json'))
            assert package['contributes']['languages'][0]['id'] == 'dctx0'
            assert package['main'] == './extension.js'
            server = Path(temporary) / ('dctx0.exe' if os.name == 'nt' else 'dctx0')
            with archive.open('extension/server/' + server.name) as source, server.open('wb') as target:
                shutil.copyfileobj(source, target)
            with server.open('rb') as installed, binary.open('rb') as original:
                assert hashlib.file_digest(installed, 'sha256').hexdigest() == hashlib.file_digest(original, 'sha256').hexdigest()
            server.chmod(0o755)
        run(['node', 'editors/vscode/test-client.cjs', str(server)])
        # The prepared Zed launcher must compile, not just contain plausible text.
        run(['cargo', 'check', '--locked', '--manifest-path', str(destination / 'zed/Cargo.toml')], env={**os.environ, 'CARGO_TARGET_DIR': str(OUT / 'zed-build')})
        assert not (destination / '.dctx0ctl').exists()
    result = {'status': 'PASS', 'checks': ['Tree-sitter corpus', 'complete Cinema syntax', 'Zed queries', 'local preview and idempotence', 'VSIX manifests and embedded native binary', 'real VS Code client/server providers', 'prepared Zed Rust launcher compilation'], 'editor_ui': 'not exercised; VS Code/Zed are not installed in this environment'}
    (OUT / 'acceptance.json').write_text(json.dumps(result, indent=2) + '\n')

def performance(binary):
    client = Client(binary, ROOT / 'examples/cinema/dctx0')
    samples = {}
    def measure(name, action):
        started = time.perf_counter()
        result = action()
        samples[name] = {'elapsed_ms': (time.perf_counter() - started) * 1000, 'peak_rss_bytes': client.rss()}
        return result
    try:
        init = measure('initialize', lambda: client.request('initialize', {'processId': None, 'capabilities': {}}))
        assert init['capabilities']['positionEncoding'] == 'utf-16'
        client.notify('initialized', {})
        file = ROOT / 'examples/cinema/dctx0/movie-night.dctx0'
        uri = file.as_uri()
        source = file.read_text()
        symbols = measure('analysis-and-symbols', lambda: client.request('textDocument/documentSymbol', {'textDocument': {'uri': uri}}))
        assert symbols
        tokens = measure('semantic-colors', lambda: client.request('textDocument/semanticTokens/full', {'textDocument': {'uri': uri}}))
        assert tokens['data']
        client.notify('textDocument/didOpen', {'textDocument': {'uri': uri, 'languageId': 'dctx0', 'version': 1, 'text': source}})
        client.request('textDocument/documentSymbol', {'textDocument': {'uri': uri}})
        def change():
            client.notify('textDocument/didChange', {'textDocument': {'uri': uri, 'version': 2}, 'contentChanges': [{'text': source + '\n// editor measurement\n'}]})
            return client.request('textDocument/documentSymbol', {'textDocument': {'uri': uri}})
        assert measure('unsaved-change', change)
        assert measure('workspace-navigation', lambda: client.request('workspace/symbol', {'query': 'FilmId'}))
    finally:
        client.close()
    result = {'status': 'PASS', 'binary': str(binary), 'samples': samples, 'memory_scope': 'Linux server process VmHWM, cumulative per persistent session; excludes editor/client process; unsupported measurement is null', 'cache': 'cold server, warm OS cache; one sample per phase; includes debounce in analysis phases'}
    (OUT / 'performance.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', type=Path, default=ROOT / 'target/release/dctx0')
    group = parser.add_mutually_exclusive_group()
    group.add_argument('--functional-only', action='store_true')
    group.add_argument('--performance-only', action='store_true')
    args = parser.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    binary = args.binary.resolve()
    if not args.performance_only:
        functional(binary)
    if not args.functional_only:
        performance(binary)
    return 0
if __name__ == '__main__':
    raise SystemExit(main())
