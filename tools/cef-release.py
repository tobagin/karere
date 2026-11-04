#!/usr/bin/env python3
"""Fail-closed CEF release selection and codec artifact checks; stdlib only."""
import argparse
import ast
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile
import urllib.request
import zipfile

POLICY = Path(__file__).with_name("cef-release-policy.json")
PLATFORMS = {"x64": ("linux64", 62), "arm64": ("linuxarm64", 183)}
VERSION = re.compile(r"(\d+)\.(\d+)\.(\d+)\+g([0-9a-f]{7,40})\+chromium-(\d+\.\d+\.\d+\.\d+)")
PATCHES = ("karere-151-detach-worksource.patch", "karere-173-pump-continuation.patch")
LEGACY_PATCH = "karere-nvidia-native-shared-images.patch"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def file_hash(path):
    with open(path, "rb") as stream:
        checksum = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            checksum.update(block)
        return checksum.hexdigest()


def fetch(url):
    request = urllib.request.Request(url, headers={"User-Agent": "karere-cef-release-check"})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def ancestry(repo, base, head, evidence):
    require(re.fullmatch(r"[0-9a-f]{40}", base or ""), "invalid minimum revision")
    url = f"https://api.github.com/repos/{repo}/compare/{base}...{head}"
    raw = fetch(url)
    evidence[url] = digest(raw)
    return json.loads(raw)["status"] in ("ahead", "identical")


def preflight(mode, version, checkout, branch, arch, policy=None):
    raw_policy = POLICY.read_bytes() if policy is None else json.dumps(policy, sort_keys=True).encode()
    policy = json.loads(raw_policy)
    minimum = policy["minimum_fixed_release"]
    require(mode != "upgrade" or minimum is not None,
            "blocked_release: no released CEF containing both required fixes has been verified")
    match = VERSION.fullmatch(version)
    require(match is not None, "expected full released CEF version")
    require(re.fullmatch(r"[0-9a-f]{40}", checkout or ""), "CEF checkout must be a full SHA")
    require(checkout.startswith(match[4]), "CEF version and checkout disagree")
    chrome = match[5]
    require(branch == chrome.split(".")[2], "CEF branch and Chromium version disagree")
    evidence = {"policy_sha256": digest(raw_policy)}
    if mode == "legacy-current-pin":
        legacy = policy["legacy_release"]
        require(all((version, checkout, branch)[i] == legacy[key] for i, key in
                    enumerate(("cef_version", "cef_checkout", "cef_branch"))),
                "legacy-current-pin permits only the current production pin")
        chromium_revision = legacy["chromium_revision"]
    else:
        floor = VERSION.fullmatch(minimum["cef_version"])
        require(floor is not None and tuple(map(int, match.group(1, 2, 3))) >=
                tuple(map(int, floor.group(1, 2, 3))), "CEF release is below the verified minimum")
        require(minimum.get("evidence"), "minimum release needs reviewed fix evidence URLs")
        platform = PLATFORMS[arch][0]
        catalog_raw = fetch("https://cef-builds.spotifycdn.com/index.json")
        catalog = json.loads(catalog_raw)[platform]["versions"]
        release = next((v for v in catalog if v["cef_version"] == version and
                        v.get("channel") == "stable"), None)
        require(release is not None and any(f["type"] == "minimal" for f in release["files"]),
                f"no official stable minimal release for {platform} and {version}")
        require(release["chromium_version"] == chrome, "official Chromium version mismatch")
        evidence["catalog_release_sha256"] = digest(json.dumps(release, sort_keys=True).encode())
        raw = fetch(f"https://api.github.com/repos/chromiumembedded/cef/commits/{match[4]}")
        require(json.loads(raw)["sha"] == checkout, "official CEF revision mismatch")
        evidence["cef_commit_sha256"] = digest(raw)
        raw = fetch(f"https://raw.githubusercontent.com/chromiumembedded/cef/{checkout}/CHROMIUM_BUILD_COMPATIBILITY.txt")
        require(ast.literal_eval(raw.decode())["chromium_checkout"] == f"refs/tags/{chrome}",
                "CEF Chromium compatibility mismatch")
        evidence["compatibility_sha256"] = digest(raw)
        raw = fetch(f"https://chromium.googlesource.com/chromium/src/+/refs/tags/{chrome}?format=JSON")
        chromium_revision = json.loads(raw.decode().removeprefix(")]}'\n"))["commit"]
        require(re.fullmatch(r"[0-9a-f]{40}", chromium_revision), "invalid Chromium revision")
        evidence["chromium_commit_sha256"] = digest(raw)
        # Later branches may contain the upstream fix without descending from a
        # previous release branch's backport commit.
        require(any(ancestry("chromiumembedded/cef", base, checkout, evidence) for base in
                    (minimum["cef_checkout"], minimum["cef_fix_commit"])), "CEF fix ancestry missing")
        require(any(ancestry("chromium/chromium", base, chromium_revision, evidence) for base in
                    (minimum["chromium_revision"], policy["chromium_fix_commit"])), "Chromium fix ancestry missing")
    return dict(status="ready", mode=mode, cef_version=version, cef_checkout=checkout,
                cef_branch=branch, chromium_version=chrome, chromium_revision=chromium_revision,
                arch=arch, minimum_fixed_release=minimum, provenance=evidence)


def command(*args, cwd):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def source_provenance(info, src, patch_dir):
    require(info["provenance"]["policy_sha256"] == file_hash(POLICY), "release policy changed; rerun preflight")
    require(command("git", "rev-parse", "HEAD", cwd=src / "cef") == info["cef_checkout"], "CEF source revision mismatch")
    require(command("git", "rev-parse", "HEAD", cwd=src) == info["chromium_revision"], "Chromium source revision mismatch")
    # CEF's version uses branch history. Shallow checkouts previously generated
    # plausible but incorrect 156.0.0-HEAD.1 headers; fail before compiling those.
    generated = command(sys.executable, "-c",
                        "from cef_version import VersionFormatter; print(VersionFormatter().get_version_string())",
                        cwd=src / "cef" / "tools")
    require(generated == info["cef_version"], "generated CEF version mismatch; fetch full CEF branch history")
    args = (src / "out" / f"Release_GN_{info['arch']}" / "args.gn").read_text()
    for key, value in (("proprietary_codecs", "true"), ("ffmpeg_branding", '"Chrome"'),
                       ("target_cpu", f'"{info["arch"]}"')):
        values = re.findall(rf"^\s*{key}\s*=\s*([^\n#]+)", args, re.M)
        require(values and values[-1].strip() == value, f"GN setting mismatch: {key}")
    names = (*PATCHES, LEGACY_PATCH) if info["mode"] == "legacy-current-pin" else PATCHES
    require({p.name for p in patch_dir.glob("*.patch")} == {*PATCHES, LEGACY_PATCH}, "unreviewed CEF patch inventory")
    return dict(info, gn_args_sha256=digest(args.encode()), patches={n: file_hash(patch_dir / n) for n in names},
                source_diff_sha256={name: digest(command("git", "diff", "HEAD", cwd=path).encode())
                                    for name, path in (("cef", src / "cef"), ("chromium", src))})


def archive_name(info, archive_format):
    return f"cef_binary_{info['cef_version']}_{PLATFORMS[info['arch']][0]}_minimal.{archive_format}"


def verify_archive(info, archive):
    require(archive.name in (archive_name(info, "zip"), archive_name(info, "tar.bz2")), "archive version/architecture/name mismatch")
    root = archive.name.removesuffix(".zip").removesuffix(".tar.bz2")
    is_zip = archive.suffix == ".zip"
    with zipfile.ZipFile(archive) if is_zip else tarfile.open(archive, "r:bz2") as container:
        entries = container.infolist() if is_zip else container.getmembers()
        files = {}
        for entry in entries:
            name = entry.filename if is_zip else entry.name
            parts = PurePosixPath(name).parts
            require(parts and parts[0] == root and ".." not in parts and "\\" not in name, "unsafe archive path")
            if (entry.is_dir() if is_zip else entry.isdir()):
                continue
            require((entry.external_attr >> 16) & 0o170000 != 0o120000 if is_zip else entry.isfile(), "archive contains links/special files")
            require(name not in files, "duplicate archive member")
            files[name] = entry

        def stream(relative):
            name = f"{root}/{relative}"
            require(name in files, f"missing artifact resource: {relative}")
            return container.open(files[name]) if is_zip else container.extractfile(files[name])

        required = ("include/cef_version.h", "include/cef_api_hash.h", "include/cef_app.h",
                    "Resources/icudtl.dat", "Resources/resources.pak", "Resources/chrome_100_percent.pak",
                    "Resources/chrome_200_percent.pak", "Resources/locales/en-US.pak",
                    "Release/v8_context_snapshot.bin", "Release/chrome-sandbox", "Release/libcef.so")
        hashes = {}
        for relative in required:
            with stream(relative) as content:
                first = content.read(64)
                require(first, f"empty artifact resource: {relative}")
                if relative in ("Release/libcef.so", "Release/chrome-sandbox"):
                    require(first[:6] == b"\x7fELF\x02\x01" and int.from_bytes(first[18:20], "little") == PLATFORMS[info["arch"]][1],
                            f"wrong ELF architecture: {relative}")
                checksum = hashlib.sha256(first)
                for block in iter(lambda: content.read(1024 * 1024), b""):
                    checksum.update(block)
                hashes[relative] = checksum.hexdigest()
        with stream("include/cef_version.h") as content:
            header = content.read().decode()
        for key, value in (("CEF_VERSION", info["cef_version"]), ("CEF_COMMIT_HASH", info["cef_checkout"])):
            require(re.search(rf'^#define {key} "{re.escape(value)}"$', header, re.M), f"header mismatch: {key}")
        for key, value in zip(("MAJOR", "MINOR", "BUILD", "PATCH"), info["chromium_version"].split(".")):
            require(re.search(rf"^#define CHROME_VERSION_{key} {value}$", header, re.M), f"Chromium header mismatch: {key}")
    return dict(info, artifact=str(archive.resolve()), archive_sha256=file_hash(archive), resource_sha256=hashes)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    pre = commands.add_parser("preflight")
    pre.add_argument("--mode", choices=("upgrade", "legacy-current-pin"), default="upgrade")
    for option in ("cef-version", "cef-checkout", "cef-branch"):
        pre.add_argument(f"--{option}", required=True)
    pre.add_argument("--arch", choices=PLATFORMS, default="x64")
    source = commands.add_parser("source")
    source.add_argument("--preflight", type=Path, required=True)
    source.add_argument("--chromium-src", type=Path, required=True)
    source.add_argument("--patch-dir", type=Path, required=True)
    artifact = commands.add_parser("artifact")
    artifact.add_argument("--provenance", type=Path, required=True)
    artifact.add_argument("--distrib-dir", type=Path, required=True)
    artifact.add_argument("--format", choices=("zip", "tar.bz2"), default="tar.bz2")
    args = parser.parse_args()
    try:
        if args.command == "preflight":
            result = preflight(args.mode, args.cef_version, args.cef_checkout, args.cef_branch, args.arch)
        elif args.command == "source":
            result = source_provenance(json.loads(args.preflight.read_text()), args.chromium_src, args.patch_dir)
        else:
            info = json.loads(args.provenance.read_text())
            require("patches" in info and "gn_args_sha256" in info, "artifact needs generated source provenance")
            path = args.distrib_dir / archive_name(info, args.format)
            result = verify_archive(info, path)
            path.with_name(path.name + ".sha256").write_text(f"{result['archive_sha256']}  {path.name}\n")
            path.with_name(path.name + ".provenance.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps(result, sort_keys=True))
        return 0
    except (ValueError, KeyError, OSError, subprocess.CalledProcessError, tarfile.TarError, zipfile.BadZipFile) as error:
        print(json.dumps(dict(status="blocked_release" if "blocked_release:" in str(error) else "error", reason=str(error))))
        return 2


if __name__ == "__main__":
    sys.exit(main())
