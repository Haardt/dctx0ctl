#!/usr/bin/env python3
"""Enforce complete merged source-line and LLVM function coverage."""
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


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
    output = ROOT / 'target/coverage'
    output.mkdir(parents=True, exist_ok=True)
    # Functions retain LLVM's strict 100% gate. LCOV counts merge source lines
    # across separately instrumented cfg(test)/library instantiations.
    instrumented = subprocess.run(['cargo', 'llvm-cov', '--workspace', '--all-features', '--locked',
                    '--fail-under-functions', '100'], cwd=ROOT)
    lcov = output / 'workspace.lcov'
    raw = output / 'llvm.json'
    for format, path in [('lcov', lcov), ('json', raw)]:
        subprocess.run(['cargo', 'llvm-cov', 'report', f'--{format}', '--output-path', str(path)], cwd=ROOT, check=True)
    result = merged_lines(lcov.read_text())
    result['instrumented_exit_code'] = instrumented.returncode
    result['llvm_totals'] = json.loads(raw.read_text())['data'][0]['totals']
    (output / 'source-summary.json').write_text(json.dumps(result, indent=2) + '\n')
    print(f"Merged source lines: {result['covered_lines']}/{result['lines']} ({result['line_percent']:.2f}%); LLVM functions: {result['llvm_totals']['functions']['percent']:.2f}%", flush=True)
    for missing in result['uncovered_lines']:
        print(f"UNCOVERED {missing['file']}:{missing['line']}", file=sys.stderr)
    return 1 if instrumented.returncode or result['uncovered_lines'] or result['llvm_totals']['functions']['percent'] < 100 else 0


if __name__ == '__main__':
    sys.exit(main())
