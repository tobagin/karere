#!/usr/bin/env python3
"""Stage a local x64 CEF archive and an isolated Devel Flatpak manifest.

This records artifact identity, not proof of applied patches or acceleration.
The input manifest must be expanded with flatpak-builder --show-manifest.
"""
import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import shutil
import tarfile
import tempfile
from urllib.parse import unquote
import zipfile


def hashes(path):
    """Hash an artifact once, using SHA-1 only for CEF's archive metadata."""
    digests = {name: hashlib.new(name) for name in ('sha256', 'sha1')}
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            for digest in digests.values():
                digest.update(block)
    return {name: digest.hexdigest() for name, digest in digests.items()}


def unpack(archive, destination):
    """Extract one distribution without accepting paths outside staging."""
    if zipfile.is_zipfile(archive):
        with zipfile.ZipFile(archive) as package:
            for member in package.infolist():
                path = PurePosixPath(member.filename)
                if path.is_absolute() or '..' in path.parts or '\\' in member.filename:
                    raise ValueError('unsafe archive path')
                if (member.external_attr >> 16) & 0o170000 == 0o120000:
                    raise ValueError('symlink in ZIP distribution is unsupported')
            package.extractall(destination)
            # ZIP extraction otherwise discards executable bits on CEF helpers.
            for member in package.infolist():
                mode = (member.external_attr >> 16) & 0o777
                if mode:
                    destination.joinpath(member.filename).chmod(mode)
    else:
        with tarfile.open(archive) as package:
            package.extractall(destination, filter='data')
    roots = list(destination.iterdir())
    if len(roots) != 1 or not roots[0].is_dir():
        raise ValueError('expected exactly one CEF distribution directory')
    return roots[0]


def stage(archive, manifest_path, source, output):
    """Keep the normal manifest intact and expose matching library/resources."""
    manifest = json.loads(manifest_path.read_text())
    if manifest.get('id') != 'io.github.tobagin.karere.Devel':
        raise ValueError('only the separate Devel application may be staged')
    cef = next(m for m in manifest['modules'] if m['name'] == 'cef-binaries')
    app = next(m for m in manifest['modules'] if m['name'] == 'karere')
    archives = [s for s in cef['sources']
                if s.get('type') == 'archive' and s.get('only-arches') == ['x86_64']]
    metadata = [s for s in cef['sources'] if s.get('type') == 'inline'
                and s.get('only-arches') == ['x86_64']
                and s.get('dest-filename') == 'archive.json']
    if len(archives) != 1 or len(metadata) != 1 or app['sources'][0].get('type') != 'dir':
        raise ValueError('unexpected CEF or application manifest structure')
    if not (source / 'Cargo.toml').is_file():
        raise ValueError('application source directory is missing Cargo.toml')
    if output.exists():
        raise ValueError('refusing to overwrite an existing stage')
    identity = hashes(archive)
    archive_info = {'type': 'minimal', 'name': unquote(archive.name), 'sha1': identity['sha1']}
    archives[0].pop('url', None)
    archives[0].update(path=str(archive), sha256=identity['sha256'])
    metadata[0]['contents'] = json.dumps(archive_info) + '\n'
    app['sources'][0]['path'] = str(source)
    # ARM archive sources remain untouched; build this candidate with --arch=x86_64.
    output.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix='.cef-stage-', dir=output.parent) as directory:
        work = Path(directory)
        extracted = work / 'extracted'
        extracted.mkdir()
        distribution = unpack(archive, extracted)
        runtime = work / 'cef'
        runtime.mkdir()
        for name in ('include', 'libcef_dll', 'cmake'):
            shutil.copytree(distribution / name, runtime / name)
        shutil.copy2(distribution / 'CMakeLists.txt', runtime / 'CMakeLists.txt')
        for name in ('Release', 'Resources'):
            shutil.copytree(distribution / name, runtime, dirs_exist_ok=True)
        required = ('libcef.so', 'icudtl.dat', 'locales', 'include/cef_api_hash.h',
                    'include/cef_version.h')
        if not all((runtime / name).exists() for name in required):
            raise ValueError('distribution lacks matching CEF runtime resources')
        # Linux ELF64, little endian, x86-64; do not silently stage another arch.
        with (runtime / 'libcef.so').open('rb') as stream:
            header = stream.read(20)
        if header[:6] != b'\x7fELF\x02\x01' or header[18:20] != b'\x3e\x00':
            raise ValueError('libcef.so is not an x86-64 ELF library')
        (runtime / 'archive.json').write_text(json.dumps(archive_info) + '\n')
        for name in ('chrome-sandbox', 'cefsimple'):
            helper = runtime / name
            if helper.exists():
                helper.chmod(helper.stat().st_mode | 0o111)
        report = {'archive': archive.name, **identity,
                  'libcef_sha256': hashes(runtime / 'libcef.so')['sha256'],
                  'api_header_sha256': hashes(runtime / 'include/cef_api_hash.h')['sha256'],
                  'version_header': (runtime / 'include/cef_version.h').read_text(),
                  'arch': 'x86_64', 'acceleration_verified': False,
                  'note': 'Local artifact identity only; verify provenance and runtime separately.'}
        (work / 'manifest.json').write_text(json.dumps(manifest, indent=2) + '\n')
        (work / 'artifact.json').write_text(json.dumps(report, indent=2) + '\n')
        shutil.rmtree(extracted)
        # Publish only a complete stage. This never edits the installed Flatpak.
        work.rename(output)
    return {key: report[key] for key in ('archive', 'sha256', 'libcef_sha256', 'arch')}


def main():
    """Prepare an explicitly selected archive without replacing evidence."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('archive', type=Path)
    parser.add_argument('--manifest', required=True, type=Path)
    parser.add_argument('--source', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    try:
        result = stage(args.archive.resolve(strict=True), args.manifest.resolve(strict=True),
                       args.source.resolve(strict=True), args.output.resolve())
    except (OSError, ValueError, StopIteration) as error:
        parser.error(str(error))
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main()
