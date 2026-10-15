#!/usr/bin/env python3
"""Enforce 97% merged source-line and LLVM function coverage."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def source_hash():
    paths = [ROOT / 'Cargo.toml', ROOT / 'Cargo.lock']
    for directory in ('crates', 'targets'):
        for pattern in ('**/*.rs', '**/Cargo.toml'):
            paths.extend((ROOT / directory).glob(pattern))
    # Target profiles and helpers are embedded with include_str! in Rust crates.
    for pattern in ('**/*.md', '**/*.ts', '**/*.go'):
        paths.extend((ROOT / 'targets').glob(pattern))
    paths.extend((ROOT / "skills").glob("**/*.md"))
    paths.extend((ROOT / "skills").glob("**/*.yaml"))
    for pattern in ("**/*.js", "**/*.cjs", "**/*.json", "**/*.scm", "**/*.toml", "**/*.rs", "**/*.c", "**/*.h", "**/*.txt", "**/Cargo.lock"):
        paths.extend(path for path in (ROOT / "editors").glob(pattern) if "target" not in path.relative_to(ROOT / "editors").parts)
    paths.extend(path for path in (ROOT / "ui/site").rglob("*") if path.is_file() and "node_modules" not in path.parts)
    digest = hashlib.sha256()
    for path in sorted(paths):
        if path.is_file():
            digest.update(str(path.relative_to(ROOT)).encode())
            digest.update(path.read_bytes())
    return digest.hexdigest()


def merged_lines(lcov):
    """Combine counts by source file and line, retaining every zero-count line."""
    counts = {}
    source = None
    for row in lcov.splitlines():
        if row.startswith('SF:'):
            source = row[3:]
        elif row.startswith('DA:'):
            if source is None:
                raise ValueError('Line count has no source file')
            fields = row[3:].split(',')
            line, count = int(fields[0]), int(fields[1])
            if line < 1 or count < 0:
                raise ValueError('Invalid coverage line count')
            key = (source, line)
            counts[key] = counts.get(key, 0) + count
        elif row == 'end_of_record':
            source = None
    if not counts:
        raise ValueError('Coverage export contains no source lines')
    missing = sorted(key for key, count in counts.items() if count == 0)
    return {'lines': len(counts), 'covered_lines': len(counts) - len(missing),
            'line_percent': 100 * (len(counts) - len(missing)) / len(counts),
            'uncovered_lines': [{'file': file, 'line': line} for file, line in missing]}


def main():
    os.environ.setdefault("DCTX0_TEST_PYTHON", sys.executable)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--skip-performance', action='store_true', help='partial development coverage; defer benchmark conformance')
    args = parser.parse_args()
    output = ROOT / 'target/coverage'
    output.mkdir(parents=True, exist_ok=True)
    # The user-approved threshold is 97% for lines and functions. LCOV counts merge source lines
    # across separately instrumented cfg(test)/library instantiations.
    lcov = output / 'workspace.lcov'
    raw = output / 'llvm.json'
    before = source_hash()
    # Object discovery must not mix current profiles with older binaries whose
    # coverage mappings point at different source lines. The same fresh directory
    # is used for compilation and both exports; raw metrics remain unfiltered.
    with tempfile.TemporaryDirectory(prefix='build-', dir=output) as build:
        # Cargo refuses to clean an existing target directory without a valid
        # cache tag. Mark only this newly created, runner-owned directory.
        (Path(build) / 'CACHEDIR.TAG').write_text(
            'Signature: 8a477f597d28d172789f06886806bc55\n'
            '# Isolated Cargo coverage build owned by the dctx0 runner.\n')
        environment = {**os.environ, 'CARGO_TARGET_DIR': build,
                       'CARGO_LLVM_COV_TARGET_DIR': build}
        instrumented = subprocess.run(['cargo', 'llvm-cov', '--no-fail-fast', '--workspace', '--all-features', '--locked',
                        '--fail-under-functions', '97'] + (['--', '--skip', 'verification_core_benchmarks_measure_both_compiled_examples_without_native_claims'] if args.skip_performance else []), cwd=ROOT, env=environment)
        for format, path in [('lcov', lcov), ('json', raw)]:
            subprocess.run(['cargo', 'llvm-cov', 'report', f'--{format}', '--output-path', str(path)], cwd=ROOT, env=environment, check=True)
    after = source_hash()
    result = merged_lines(lcov.read_text())
    result['instrumented_exit_code'] = instrumented.returncode
    result['build_scope'] = 'fresh isolated directory; no reused instrumented binaries'
    result['source_hash_before'] = before
    result['source_hash_after'] = after
    result['coverage_scope'] = 'partial-performance-deferred' if args.skip_performance else 'workspace'
    result['llvm_totals'] = json.loads(raw.read_text())['data'][0]['totals']
    (output / 'source-summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(f"Merged source lines: {result['covered_lines']}/{result['lines']} ({result['line_percent']:.2f}%); LLVM functions: {result['llvm_totals']['functions']['percent']:.2f}%", flush=True)
    for missing in result['uncovered_lines']:
        print(f"UNCOVERED {missing['file']}:{missing['line']}", file=sys.stderr)
    if before != after:
        print('Rust compilation inputs changed during coverage; final-tree coverage is unproven', file=sys.stderr)
    return 1 if before != after or instrumented.returncode or result['line_percent'] < 97 or result['llvm_totals']['functions']['percent'] < 97 else 0


if __name__ == '__main__':
    from memory_guard import guard_script
    guard_script()
    sys.exit(main())
