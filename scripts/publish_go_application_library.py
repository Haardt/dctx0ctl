#!/usr/bin/env python3
"""Publish the pinned testing-only Go module into an application (no business writes)."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

ROOT = Path(__file__).resolve().parents[1]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--project', type=Path, default=ROOT / 'examples/cinema-go')
    args = parser.parse_args()
    root = args.project.resolve(strict=True)
    destination = root / '.dctx0/application-tests/library'
    destination.mkdir(parents=True, exist_ok=True)
    files = {}
    for source in sorted((ROOT / 'targets/go/applicationtests').iterdir()):
        if source.is_file() and (source.name == 'go.mod' or source.suffix == '.go' and not source.name.endswith('_test.go')):
            target = destination / source.name
            if target.is_symlink() or not target.resolve().is_relative_to(root):
                raise ValueError('Testing library destination escapes application')
            shutil.copyfile(source, target)
            files[source.name] = hashlib.sha256(source.read_bytes()).hexdigest()
    (destination.parent / 'library-pin.json').write_text(json.dumps(dict(module='github.com/Haardt/dctx0ctl/applicationtests/go', version='0.2.0', protocol_version=2, sha256=files), indent=2)+'\n')

if __name__ == '__main__':
    main()
