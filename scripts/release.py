#!/usr/bin/env python3
"""Version guards, deterministic release archives, and release checksum manifests."""
import argparse
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import subprocess
import tarfile
import tomllib
import zipfile
import platform
from datetime import datetime, timezone

ROOT = Path(__file__).resolve().parents[1]
TARGETS = {
    'x86_64-unknown-linux-gnu': ('elf', 62),
    'aarch64-unknown-linux-gnu': ('elf', 183),
    'x86_64-apple-darwin': ('macho', 0x01000007),
    'aarch64-apple-darwin': ('macho', 0x0100000C),
    'x86_64-pc-windows-msvc': ('pe', 0x8664),
}


def native_target():
    system = platform.system()
    architecture = platform.machine().lower()
    arm = architecture in ('arm64', 'aarch64')
    if system == 'Windows' and architecture in ('amd64', 'x86_64'):
        return 'x86_64-pc-windows-msvc'
    if system in ('Linux', 'Darwin') and (arm or architecture in ('amd64', 'x86_64')):
        return ('aarch64' if arm else 'x86_64') + ('-apple-darwin' if system == 'Darwin' else '-unknown-linux-gnu')
    raise ValueError(f'Unsupported release platform: {system}/{architecture}')


def asset_name(tag, target):
    return f'dctx0-{tag}-{target}.' + ('zip' if TARGETS[target][0] == 'pe' else 'tar.gz')


def validate_image(data, target):
    kind, machine = TARGETS[target]
    valid = False
    if kind == 'elf':
        valid = len(data) >= 64 and data[:6] == b'\x7fELF\x02\x01' and int.from_bytes(data[18:20], 'little') == machine
    elif kind == 'macho':
        valid = len(data) >= 32 and data[:4] == b'\xcf\xfa\xed\xfe' and int.from_bytes(data[4:8], 'little') == machine
    elif len(data) >= 64 and data[:2] == b'MZ':
        offset = int.from_bytes(data[60:64], 'little')
        valid = offset >= 64 and offset + 26 <= len(data) and data[offset:offset + 4] == b'PE\x00\x00' and int.from_bytes(data[offset + 4:offset + 6], 'little') == machine and data[offset + 24:offset + 26] == b'\x0b\x02'
    if not valid:
        raise ValueError(f'Binary is not the expected 64-bit {kind} image for {target}')
TAG_PATTERN = re.compile(r'^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$')


def workspace_version():
    return tomllib.loads((ROOT / 'Cargo.toml').read_text())['workspace']['package']['version']


def validate_tag(tag, expected=None):
    match = TAG_PATTERN.fullmatch(tag)
    if not match or (match[4] and any(part.isdigit() and len(part) > 1 and part.startswith('0')
                                    for part in match[4].split('.'))):
        raise ValueError('Use a SemVer tag such as v0.1.0 or v0.2.0-rc.1')
    if tag[1:] != (expected if expected is not None else workspace_version()):
        raise ValueError(f'Tag {tag} does not match Cargo workspace version {expected or workspace_version()}')
    return {'tag': tag, 'version': tag[1:], 'prerelease': bool(match[4])}


def package(binary, target, tag, output, epoch=None):
    validate_tag(tag)
    if target not in TARGETS:
        raise ValueError(f'Unsupported release target: {target}')
    data = binary.read_bytes()
    validate_image(data, target)
    revision = subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip()
    epoch = int(epoch) if epoch is not None else int(subprocess.check_output(
        ['git', 'show', '-s', '--format=%ct', 'HEAD'], cwd=ROOT, text=True).strip())
    if epoch < 0 or epoch > 0xFFFFFFFF:
        raise ValueError('Archive timestamp must fit the gzip timestamp field')
    receipt = {'schema_version': 1, 'tag': tag, 'target': target, 'revision': revision,
               'binary_sha256': hashlib.sha256(data).hexdigest(),
               'minimum_platform': 'Windows 10 x64' if TARGETS[target][0] == 'pe' else 'macOS 14' if TARGETS[target][0] == 'macho' else 'Linux glibc 2.35',
               'features': ['typescript', 'java', 'go', 'rust'],
               'rustc': subprocess.check_output(['rustc', '--version'], text=True).strip()}
    executable = 'dctx0.exe' if TARGETS[target][0] == 'pe' else 'dctx0'
    files = {executable: data, 'LICENSE': (ROOT / 'LICENSE').read_bytes(),
             'release.json': (json.dumps(receipt, indent=2) + '\n').encode(),
             "README.txt": f"DCTX0 {tag}\n\nRun {executable} --version.\nCLI, LSP, profiles and agent skills are embedded in the binary.\nNative verification additionally requires Bun, Go or Rust and application dependencies.\nTarget: {target}; {receipt['minimum_platform']}.\nNative verification, replay and guarded adoption support all five release platforms for TypeScript/Bun, Go and Rust. Linux retains process-tree supervision; macOS/Windows use bounded direct-child cleanup. Rust needs Xcode tools on macOS or an MSVC developer environment with Windows SDK on Windows.\nhttps://github.com/Haardt/dctx0ctl\n".encode()}
    output.mkdir(parents=True, exist_ok=True)
    archive = output / asset_name(tag, target)
    temporary = archive.with_suffix('.tmp')
    if TARGETS[target][0] == 'pe':
        date = datetime.fromtimestamp(max(epoch, 315532800), timezone.utc).timetuple()[:6]
        with zipfile.ZipFile(temporary, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as bundle:
            for name, payload in files.items():
                member = zipfile.ZipInfo(name, date)
                member.create_system = 3
                member.external_attr = (0o100755 if name == executable else 0o100644) << 16
                member.compress_type = zipfile.ZIP_DEFLATED
                bundle.writestr(member, payload, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    else:
        with temporary.open('wb') as destination:
            with gzip.GzipFile(filename='', mode='wb', fileobj=destination, mtime=epoch) as compressed:
                with tarfile.open(fileobj=compressed, mode='w', format=tarfile.USTAR_FORMAT) as bundle:
                    for name, payload in files.items():
                        member = tarfile.TarInfo(name)
                        member.size = len(payload)
                        member.mtime = epoch
                        member.mode = 0o755 if name == executable else 0o644
                        bundle.addfile(member, io.BytesIO(payload))
    temporary.replace(archive)
    return archive


def checksums(directory, tag):
    validate_tag(tag)
    expected = {asset_name(tag, target) for target in TARGETS} | {'install.sh', 'install.ps1'}
    actual = {path.name for path in directory.iterdir() if path.is_file()}
    if actual != expected:
        raise ValueError(f'Release assets differ from the expected set: missing {sorted(expected - actual)}, extra {sorted(actual - expected)}')
    manifest = directory / 'SHA256SUMS'
    manifest.write_text(''.join(f'{hashlib.sha256((directory / name).read_bytes()).hexdigest()}  {name}\n'
                                for name in sorted(expected)))
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    actions = parser.add_subparsers(dest='action', required=True)
    tag = actions.add_parser('validate-tag')
    tag.add_argument('tag')
    archive = actions.add_parser('package')
    archive.add_argument('--binary', type=Path, required=True)
    archive.add_argument('--target', choices=TARGETS, required=True)
    archive.add_argument('--tag', required=True)
    archive.add_argument('--output', type=Path, required=True)
    archive.add_argument('--epoch', type=int)
    manifest = actions.add_parser('checksums')
    manifest.add_argument('--directory', type=Path, required=True)
    manifest.add_argument('--tag', required=True)
    args = parser.parse_args()
    try:
        if args.action == 'validate-tag':
            print(json.dumps(validate_tag(args.tag)))
        elif args.action == 'package':
            print(package(args.binary, args.target, args.tag, args.output, args.epoch))
        else:
            print(checksums(args.directory, args.tag))
    except (OSError, ValueError) as error:
        parser.exit(1, f'Release preparation failed: {error}\n')


if __name__ == '__main__':
    main()
