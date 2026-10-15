#!/usr/bin/env python3
"""Build/check the mdBook site and record coupled release build speed/RSS samples."""
import argparse
from datetime import datetime, timezone
import fcntl
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import statistics
import sys
import subprocess
import tempfile
import time
from urllib.parse import unquote, urlsplit
from memory_guard import guarded_run, systemd_available

ROOT = Path(__file__).resolve().parents[1]
BOOK = ROOT / 'docs/site'
OUTPUT = ROOT / 'target/docs-site'
HISTORY = ROOT / 'benchmarks/history/docs-site'
VERSION = '0.5.4'


class Document(HTMLParser):
    def __init__(self, text):
        super().__init__(convert_charrefs=True)
        self.links = []
        self.ids = set()
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if values.get('id'):
            self.ids.add(values['id'])
        for key in ('href', 'src'):
            if values.get(key):
                self.links.append(values[key])


def measured(command, timeout=120, env=None):
    started = time.perf_counter()
    with tempfile.TemporaryFile() as log:
        process = subprocess.Popen(command, cwd=ROOT, stdout=log, stderr=log,
                                   start_new_session=True, env=env)
        usage = None
        try:
            if hasattr(os, 'wait4'):
                while True:
                    pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                    if pid:
                        process.returncode = os.waitstatus_to_exitcode(status)
                        break
                    if time.perf_counter() - started > timeout:
                        raise TimeoutError(f'Timed out: {command[0]}')
                    time.sleep(0.01)
            else:
                process.wait(timeout=timeout)
        finally:
            if process.returncode is None:
                os.killpg(process.pid, signal.SIGKILL)
                if hasattr(os, 'wait4'):
                    _, status, usage = os.wait4(process.pid, 0)
                    process.returncode = os.waitstatus_to_exitcode(status)
                else:
                    process.wait()
        elapsed = (time.perf_counter() - started) * 1000
        log.seek(0)
        output = log.read().decode(errors='replace')
    rss = usage.ru_maxrss * 1024 if usage is not None and platform.system() == 'Linux' else None
    return {'elapsed_ms': elapsed, 'peak_rss_bytes': rss,
            'exit_code': process.returncode}, output


def require(command, label, measurements):
    sample, output = measured(command)
    measurements.setdefault(label, []).append(sample)
    if sample['exit_code']:
        raise RuntimeError(f'{label} failed:\n{output[:12000]}')
    return output


def validate_html():
    documents = {path: Document(path.read_text()) for path in OUTPUT.rglob('*.html')}
    failures = []
    checked = 0
    for path, document in documents.items():
        for href in document.links:
            url = urlsplit(href)
            if url.scheme or url.netloc:
                continue
            # mdBook's 404 assets use the configured project-site prefix.
            target_path = unquote(url.path)
            if target_path.startswith('/dctx0ctl/'):
                target = OUTPUT / target_path[len('/dctx0ctl/'):]
            elif target_path.startswith('/'):
                failures.append(f'{path.relative_to(OUTPUT)}: root-relative URL {href}')
                continue
            else:
                target = path.parent / target_path if target_path else path
            target = target.resolve()
            if not target.is_relative_to(OUTPUT.resolve()):
                failures.append(f'{path.relative_to(OUTPUT)}: escaping URL {href}')
                continue
            if target.is_dir():
                target /= 'index.html'
            checked += 1
            if not target.is_file():
                failures.append(f'{path.relative_to(OUTPUT)}: missing {href}')
            elif url.fragment and target in documents and unquote(url.fragment) not in documents[target].ids:
                failures.append(f'{path.relative_to(OUTPUT)}: missing anchor {href}')
    for path in OUTPUT.rglob('*.css'):
        for href in re.findall(r'url\([\'"]?([^\)\'\"]+)', path.read_text()):
            if not urlsplit(href).scheme and not href.startswith('data:'):
                target = (path.parent / unquote(href)).resolve()
                checked += 1
                if not target.is_file():
                    failures.append(f'{path.relative_to(OUTPUT)}: missing CSS asset {href}')
    if failures:
        raise RuntimeError('Site link validation failed:\n' + '\n'.join(failures[:40]))
    return {'html_pages': len(documents), 'local_links_and_assets': checked,
            'external_links': 'not fetched; repository/source references are authored URLs'}


def validate_specs(binary, measurements):
    checked = []
    for source in sorted((BOOK / 'src').rglob('*.md')):
        blocks = re.findall(r'<!-- dctx0:check -->\s*```dctx0\n(.*?)\n```', source.read_text(), re.S)
        for index, block in enumerate(blocks):
            with tempfile.TemporaryDirectory(prefix='site-spec-', dir=ROOT / 'target') as directory:
                path = Path(directory)
                (path / 'example.dctx0').write_text(block + '\n')
                require([str(binary), 'check', str(path), '--json'],
                        'specification-check', measurements)
            checked.append(f'{source.relative_to(BOOK)}:{index + 1}')
    if not checked:
        raise RuntimeError('No compiler-checked complete documentation examples found.')
    return checked


def provenance():
    digest = hashlib.sha256()
    inputs = sorted(p for p in BOOK.rglob('*') if p.is_file()) + [ROOT / 'scripts' / name for name in ('docs_site.py', 'docs_site_browser.ts', 'install_mdbook.py', 'memory_guard.py')]
    inputs.append(ROOT / '.github/workflows/pages.yml')
    for path in inputs:
        digest.update(str(path.relative_to(ROOT)).encode())
        digest.update(path.read_bytes())
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    dirty = bool(subprocess.check_output(['git', 'status', '--porcelain'], cwd=ROOT, text=True).strip())
    return {'revision': revision, 'dirty_worktree': dirty, 'content_hash': digest.hexdigest(),
            'platform': platform.platform(), 'machine': platform.machine(),
            'memory_guard': {'active': os.environ.get('DCTX0_MEMORY_GUARDED') == '1',
                             'budget_bytes': int(os.environ['DCTX0_MEMORY_BUDGET_BYTES']) if os.environ.get('DCTX0_MEMORY_BUDGET_BYTES') else None},
            'cpu': next((line.split(':', 1)[1].strip() for line in Path('/proc/cpuinfo').read_text().splitlines()
                         if line.startswith('model name')), 'unknown') if Path('/proc/cpuinfo').exists() else 'unknown'}


def save(record):
    HISTORY.mkdir(parents=True, exist_ok=True)
    path = HISTORY / (record['id'] + '.json')
    with (HISTORY.parent / '.docs-site.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        with tempfile.NamedTemporaryFile(mode='w', dir=HISTORY, prefix='.write-', delete=False) as staged:
            json.dump(record, staged, indent=2)
            staged.write('\n')
            staged.flush()
            os.fsync(staged.fileno())
        os.replace(staged.name, path)
        for old in sorted(HISTORY.glob('*.json'))[:-20]:
            old.unlink()
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--mdbook', type=Path, default=ROOT / 'target/docs-tools/mdbook')
    parser.add_argument('--dctx0', type=Path, default=ROOT / 'target/release/dctx0')
    parser.add_argument('--samples', type=int, default=3)
    parser.add_argument('--browser', action='store_true', help='also check desktop/mobile/search with Bun and an existing Chromium')
    args = parser.parse_args()
    if args.samples < 1:
        parser.error('At least one measurement sample is required.')
    args.mdbook = args.mdbook.resolve() if args.mdbook.exists() else Path(shutil.which('mdbook') or args.mdbook)
    if not args.mdbook.is_file() or not args.dctx0.is_file():
        parser.error('Install mdBook and build dctx0 first; select binaries with --mdbook and --dctx0.')
    args.dctx0 = args.dctx0.resolve()
    version = subprocess.check_output([str(args.mdbook), '--version'], text=True).strip()
    if version != f'mdbook v{VERSION}':
        parser.error(f'Expected mdbook v{VERSION}, found {version}.')
    ROOT.joinpath('target').mkdir(exist_ok=True)
    now = datetime.now(timezone.utc)
    record = {'schema_version': 1, 'id': now.strftime('%Y%m%dT%H%M%S.%fZ') + f'-{os.getpid()}',
              'timestamp': now.isoformat(), 'workload': 'documentation-site', 'status': 'incomplete',
              'metadata': provenance(), 'mdbook': version,
              'dctx0_sha256': hashlib.sha256(args.dctx0.read_bytes()).hexdigest(),
              'memory_scope': 'Linux wait4 ru_maxrss of each child process in bytes; excludes Python/orchestration; other platforms report null',
              'cache': 'one warm-up build, fresh mdBook process per sample, warm filesystem cache',
              'seed': None, 'features': 'mdBook official release HTML/search renderer',
              'baseline_comparison': {'status': 'unassessed', 'reason': 'No accepted documentation baseline; core baseline unchanged.'},
              'measurements': {}}
    failure = None
    try:
        require([str(args.mdbook), 'build', str(BOOK)], 'build-validation', record['measurements'])
        record['validation'] = validate_html()
        record['validation']['complete_specs'] = validate_specs(args.dctx0, record['measurements'])
        if args.browser:
            with tempfile.TemporaryDirectory(prefix='docs-browser-', dir=ROOT / 'target') as directory:
                browser_report = Path(directory) / 'report.json'
                require(['bun', str(ROOT / 'scripts/docs_site_browser.ts'), str(browser_report)],
                        'browser-ui', record['measurements'])
                result = json.loads(browser_report.read_text())
                if not isinstance(result, dict) or result.get('status') != 'PASS':
                    raise RuntimeError('Browser acceptance did not supply a successful report')
                record['validation']['browser'] = result
    except (OSError, RuntimeError, TimeoutError, ValueError) as error:
        failure = str(error)
    try:
        # Functional checks first; dedicated release performance samples last, including failed validation.
        require([str(args.mdbook), 'build', str(BOOK)], 'build-warmup', record['measurements'])
        for _ in range(args.samples):
            require([str(args.mdbook), 'build', str(BOOK)], 'release-build', record['measurements'])
        samples = record['measurements']['release-build']
        memory = [sample['peak_rss_bytes'] for sample in samples if sample['peak_rss_bytes'] is not None]
        record['summary'] = {'samples': len(samples),
                             'median_elapsed_ms': statistics.median(sample['elapsed_ms'] for sample in samples),
                             'peak_rss_bytes': max(memory) if len(memory) == len(samples) else None}
        record['status'] = 'failed' if failure else 'complete'
        if failure:
            record['failure'] = failure
    except (OSError, RuntimeError, TimeoutError) as error:
        failure = (failure + '\n' if failure else '') + str(error)
        record['status'] = 'failed'
        record['failure'] = failure
    finally:
        result = save(record)
        print(json.dumps({'status': record['status'], 'report': str(result.relative_to(ROOT)),
                          'site': str(OUTPUT.relative_to(ROOT)), 'summary': record.get('summary'),
                          'validation': record.get('validation'), 'failure': failure}, indent=2))
    return 1 if failure else 0


if __name__ == '__main__':
    if os.environ.get('DCTX0_MEMORY_GUARDED') != '1':
        if os.environ.get('DCTX0_MEMORY_GUARD') != '0' and systemd_available():
            raise SystemExit(guarded_run([sys.executable, *sys.argv]))
        print('Documentation memory guard disabled or unavailable; no collective RAM cap.', file=sys.stderr)
    ROOT.joinpath('target').mkdir(exist_ok=True)
    with (ROOT / 'target/docs-site.lock').open('w') as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        raise SystemExit(main())
