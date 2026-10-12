#!/usr/bin/env python3
"""Install the pinned, checksum-verified Linux x86_64 mdBook release locally."""
import hashlib
import io
from pathlib import Path
import platform
import tarfile
import urllib.request

VERSION = '0.5.4'
SHA256 = '3f28de05dafca9d0f2eab99c662116b0e37b89b1d96a08f8f430b9eeae958cd7'
ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / 'target' / 'docs-tools' / 'mdbook'


def main():
    if platform.system() != 'Linux' or platform.machine() not in ('x86_64', 'AMD64'):
        raise SystemExit(f'Use cargo install mdbook --version {VERSION} --locked on this platform.')
    archive = f'mdbook-v{VERSION}-x86_64-unknown-linux-gnu.tar.gz'
    url = f'https://github.com/rust-lang/mdBook/releases/download/v{VERSION}/{archive}'
    with urllib.request.urlopen(url, timeout=60) as response:
        payload = response.read(16 * 1024 * 1024 + 1)
    if hashlib.sha256(payload).hexdigest() != SHA256:
        raise SystemExit('mdBook archive checksum mismatch; refusing installation.')
    with tarfile.open(fileobj=io.BytesIO(payload), mode='r:gz') as bundle:
        member = bundle.getmember('mdbook')
        if not member.isfile():
            raise SystemExit('mdBook archive has no regular executable.')
        binary = bundle.extractfile(member).read()
    DESTINATION.parent.mkdir(parents=True, exist_ok=True)
    staged = DESTINATION.with_suffix('.tmp')
    staged.write_bytes(binary)
    staged.chmod(0o755)
    staged.replace(DESTINATION)
    print(f'Installed mdBook {VERSION}: {DESTINATION}')


if __name__ == '__main__':
    main()
