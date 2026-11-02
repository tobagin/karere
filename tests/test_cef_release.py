#!/usr/bin/env python3
"""Offline fixtures for release gating and exact codec artifact selection."""
import copy
import importlib.util
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("cef_release", ROOT / "tools/cef-release.py")
cef = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cef)
POLICY = json.loads(cef.POLICY.read_text())
LEGACY = POLICY["legacy_release"]


def legacy(arch="x64"):
    return cef.preflight("legacy-current-pin", LEGACY["cef_version"], LEGACY["cef_checkout"], "7977", arch)


class ReleaseTests(unittest.TestCase):
    def test_pending_release_blocks_before_network(self):
        with patch.object(cef, "fetch", side_effect=AssertionError("must not fetch")):
            with self.assertRaisesRegex(ValueError, "blocked_release"):
                cef.preflight("upgrade", "", "", "", "x64")

    def test_legacy_requires_exact_pin_without_network(self):
        with patch.object(cef, "fetch", side_effect=AssertionError("must not fetch")):
            self.assertEqual(legacy()["chromium_revision"], LEGACY["chromium_revision"])
            with self.assertRaisesRegex(ValueError, "legacy-current-pin"):
                cef.preflight("legacy-current-pin", LEGACY["cef_version"].replace("152.0.6", "152.0.7"),
                              LEGACY["cef_checkout"], "7977", "arm64")

    def test_checkout_and_branch_must_match_version(self):
        for sha, branch in (("708dc14", "7977"), ("a" * 40, "7977"), (LEGACY["cef_checkout"], "8078")):
            with self.subTest(sha=sha, branch=branch), self.assertRaises(ValueError):
                cef.preflight("legacy-current-pin", LEGACY["cef_version"], sha, branch, "x64")

    def test_shell_pending_gate_leaves_cache_untouched(self):
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory) / "not-created"
            env = dict(os.environ, CEF_BUILD_MODE="upgrade", CEF_BRANCH="8078")
            result = subprocess.run(["bash", str(ROOT / "tools/build-cef-codecs.sh"), str(cache)],
                                    env=env, text=True, capture_output=True)
            self.assertEqual(result.returncode, 2)
            self.assertIn("blocked_release", result.stderr)
            self.assertFalse(cache.exists())

    def fixed_fixture(self, *, missing=False, status="ahead", later_branch=False):
        # Entirely synthetic future versions; these are not release claims.
        policy = copy.deepcopy(POLICY)
        version = "159.0.2+gaaaaaaa+chromium-159.0.9001.2"
        policy["minimum_fixed_release"] = dict(
            cef_version="158.0.1+gbbbbbbb+chromium-158.0.9000.1",
            cef_checkout="b" * 40, cef_fix_commit="c" * 40,
            chromium_revision="d" * 40, evidence=["https://example.invalid/fixture-only"])
        release = dict(cef_version=version, chromium_version="159.0.9001.2", channel="stable",
                       files=[dict(type="minimal")])

        def fake_fetch(url):
            if url.endswith("index.json"):
                return json.dumps({p: {"versions": [] if missing else [release]} for p, _ in cef.PLATFORMS.values()}).encode()
            if "compare/" in url:
                selected = "diverged" if later_branch and ("/compare/" + "b" * 40 in url or "/compare/" + "d" * 40 in url) else status
                return json.dumps({"status": selected}).encode()
            if "/commits/" in url:
                return json.dumps({"sha": "a" * 40}).encode()
            if url.endswith("CHROMIUM_BUILD_COMPATIBILITY.txt"):
                return b"{'chromium_checkout': 'refs/tags/159.0.9001.2'}"
            return b")]}'\n" + json.dumps({"commit": "e" * 40}).encode()
        return policy, version, fake_fetch

    def test_verified_newer_stable_release_accepts_both_architectures(self):
        policy, version, fetch = self.fixed_fixture()
        with patch.object(cef, "fetch", side_effect=fetch):
            for arch in cef.PLATFORMS:
                info = cef.preflight("upgrade", version, "a" * 40, "9001", arch, policy)
                self.assertEqual(info["status"], "ready")
                self.assertEqual(info["chromium_revision"], "e" * 40)
                self.assertIn("catalog_release_sha256", info["provenance"])

    def test_newer_branch_may_contain_upstream_fixes(self):
        policy, version, fetch = self.fixed_fixture(later_branch=True)
        with patch.object(cef, "fetch", side_effect=fetch):
            self.assertEqual(cef.preflight("upgrade", version, "a" * 40, "9001", "x64", policy)["status"], "ready")

    def test_unreleased_or_missing_fix_is_rejected(self):
        for kwargs in ({"missing": True}, {"status": "diverged"}):
            policy, version, fetch = self.fixed_fixture(**kwargs)
            with self.subTest(kwargs=kwargs), patch.object(cef, "fetch", side_effect=fetch), self.assertRaises(ValueError):
                cef.preflight("upgrade", version, "a" * 40, "9001", "x64", policy)

    def test_below_minimum_rejected_before_network(self):
        policy, _, _ = self.fixed_fixture()
        with patch.object(cef, "fetch", side_effect=AssertionError("must not fetch")), self.assertRaisesRegex(ValueError, "below"):
            cef.preflight("upgrade", LEGACY["cef_version"], LEGACY["cef_checkout"], "7977", "x64", policy)


class ArtifactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.directory = Path(self.temp.name)

    def make_archive(self, info, format="zip", omit=None, wrong_machine=False, wrong_header=False, unsafe=False):
        archive = self.directory / cef.archive_name(info, format)
        prefix = archive.name.removesuffix(".zip").removesuffix(".tar.bz2")
        header = f'#define CEF_VERSION "{info["cef_version"]}"\n#define CEF_COMMIT_HASH "{info["cef_checkout"]}"\n'
        for key, value in zip(("MAJOR", "MINOR", "BUILD", "PATCH"), info["chromium_version"].split(".")):
            header += f"#define CHROME_VERSION_{key} {value}\n"
        if wrong_header:
            header = header.replace("152.0.6", "152.0.0-HEAD.1")
        binary = bytearray(64)
        binary[:6] = b"\x7fELF\x02\x01"
        binary[18:20] = (183 if wrong_machine else cef.PLATFORMS[info["arch"]][1]).to_bytes(2, "little")
        members = {n: b"fixture" for n in ("include/cef_api_hash.h", "include/cef_app.h", "Resources/icudtl.dat",
                   "Resources/resources.pak", "Resources/chrome_100_percent.pak", "Resources/chrome_200_percent.pak",
                   "Resources/locales/en-US.pak", "Release/v8_context_snapshot.bin")}
        members.update({"include/cef_version.h": header.encode(), "Release/libcef.so": bytes(binary), "Release/chrome-sandbox": bytes(binary)})
        if omit:
            del members[omit]
        if unsafe:
            members["../escape"] = b"unsafe"
        if format == "zip":
            with zipfile.ZipFile(archive, "w") as out:
                for name, data in members.items():
                    out.writestr(f"{prefix}/{name}", data)
        else:
            with tarfile.open(archive, "w:bz2") as out:
                for name, data in members.items():
                    entry = tarfile.TarInfo(f"{prefix}/{name}")
                    entry.size = len(data)
                    out.addfile(entry, io.BytesIO(data))
        return archive

    def test_both_architectures_and_formats(self):
        for arch in cef.PLATFORMS:
            for format in ("zip", "tar.bz2"):
                with self.subTest(arch=arch, format=format):
                    info = legacy(arch)
                    path = self.make_archive(info, format)
                    result = cef.verify_archive(info, path)
                    self.assertEqual(result["archive_sha256"], cef.file_hash(path))
                    self.assertEqual(len(result["resource_sha256"]), 11)

    def test_wrong_headers_architecture_resources_and_paths_fail(self):
        info = legacy()
        for kwargs in ({"wrong_header": True}, {"wrong_machine": True}, {"omit": "Resources/icudtl.dat"}, {"unsafe": True}):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                cef.verify_archive(info, self.make_archive(info, **kwargs))

    def test_wrong_filename_architecture_rejected(self):
        path = self.make_archive(legacy("arm64"))
        with self.assertRaisesRegex(ValueError, "name mismatch"):
            cef.verify_archive(legacy("x64"), path)

    def test_artifact_command_emits_sidecars_and_ignores_other_versions(self):
        info = dict(legacy(), patches={"fixture": "hash"}, gn_args_sha256="fixture")
        path = self.make_archive(info, "tar.bz2")
        (self.directory / "cef_binary_unrelated_linuxarm64_minimal.zip").write_text("not an archive")
        provenance = self.directory / "source.json"
        provenance.write_text(json.dumps(info))
        result = subprocess.run(["python3", str(ROOT / "tools/cef-release.py"), "artifact", "--provenance",
                                 str(provenance), "--distrib-dir", str(self.directory)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        data = json.loads(result.stdout)
        self.assertEqual(data["artifact"], str(path))
        self.assertEqual(path.with_name(path.name + ".sha256").read_text(), f"{cef.file_hash(path)}  {path.name}\n")
        self.assertEqual(json.loads(path.with_name(path.name + ".provenance.json").read_text()), data)


class SourceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.src = Path(self.temp.name)
        self.args = self.src / "out/Release_GN_x64/args.gn"
        self.args.parent.mkdir(parents=True)
        self.args.write_text('proprietary_codecs=true\nffmpeg_branding="Chrome"\ntarget_cpu="x64"\n')
        self.patch_dir = self.src / "patches"
        self.patch_dir.mkdir()
        for name in (*cef.PATCHES, cef.LEGACY_PATCH):
            (self.patch_dir / name).write_text("fixture patch")

    def fake_command(self, *args, cwd):
        if args[0] == sys.executable:
            return LEGACY["cef_version"]
        if args[1] == "rev-parse":
            return LEGACY["cef_checkout"] if cwd.name == "cef" else LEGACY["chromium_revision"]
        return "fixture source diff"

    def check(self, info=None):
        return cef.source_provenance(info or legacy(), self.src, self.patch_dir)

    def test_source_records_revision_gn_and_patch_provenance(self):
        with patch.object(cef, "command", side_effect=self.fake_command):
            result = self.check()
            self.assertEqual(set(result["patches"]), {*cef.PATCHES, cef.LEGACY_PATCH})
            self.assertEqual(result["gn_args_sha256"], cef.file_hash(self.args))
            self.assertEqual(set(result["source_diff_sha256"]), {"cef", "chromium"})

    def test_shallow_wrong_version_and_unexpected_patch_fail(self):
        def shallow_version(*args, cwd):
            return "152.0.0-HEAD.1" if args[0] == sys.executable else self.fake_command(*args, cwd=cwd)
        with patch.object(cef, "command", side_effect=shallow_version), self.assertRaisesRegex(ValueError, "generated CEF version"):
            self.check()
        (self.patch_dir / "unreviewed.patch").write_text("extra")
        with patch.object(cef, "command", side_effect=self.fake_command), self.assertRaisesRegex(ValueError, "inventory"):
            self.check()

    def test_disabled_codecs_wrong_arch_or_changed_policy_fail(self):
        for assignment in ('proprietary_codecs=false', 'ffmpeg_branding="Chromium"', 'target_cpu="arm64"'):
            with self.subTest(assignment=assignment):
                original = self.args.read_text()
                self.args.write_text(original + assignment + "\n")
                with patch.object(cef, "command", side_effect=self.fake_command), self.assertRaisesRegex(ValueError, "GN setting"):
                    self.check()
                self.args.write_text(original)
        info = legacy()
        info["provenance"]["policy_sha256"] = "wrong"
        with self.assertRaisesRegex(ValueError, "policy changed"):
            self.check(info)


if __name__ == "__main__":
    unittest.main()
