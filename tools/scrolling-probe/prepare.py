#!/usr/bin/env python3
"""Build the version-specific probes and isolated schemas for Karere Flatpak."""
import os
from pathlib import Path
import platform
import shlex
import shutil
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parent
APP = "io.github.tobagin.karere"


def main():
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise SystemExit("The CEF ABI in these probes was verified only on Linux x86-64.")
    compiler = shlex.split(os.environ.get("CC", "gcc"))
    for command in (compiler[0], "flatpak", "glib-compile-schemas", "gapplication"):
        if not shutil.which(command):
            raise SystemExit(f"Missing required command: {command}")
    schema = subprocess.check_output([
        "flatpak", "run", "--command=cat", APP,
        f"/app/share/glib-2.0/schemas/{APP}.gschema.xml",
    ])
    build = ROOT / ".build"
    build.mkdir(exist_ok=True)
    environment = dict(os.environ, CCACHE_DISABLE="1")
    with tempfile.TemporaryDirectory(dir=build) as temporary:
        for name in ("pump_probe", "schedule_probe"):
            output = Path(temporary) / f"{name}.so"
            subprocess.run(compiler + [
                "-shared", "-fPIC", "-O2", "-mtls-dialect=gnu",
                "-Wall", "-Wextra", "-Werror", str(ROOT / f"{name}.c"),
                "-o", str(output), "-ldl",
            ], check=True, env=environment)
            # Replace the inode rather than truncating a potentially mapped library.
            output.replace(build / output.name)
    for gpu in (False, True):
        directory = build / ("schemas_gpu" if gpu else "schemas")
        directory.mkdir(exist_ok=True)
        (directory / f"{APP}.gschema.xml").write_bytes(schema)
        (directory / "99-perf.gschema.override").write_text(
            f"[{APP}]\nis-maximized=true\ngpu-rendering={str(gpu).lower()}\n"
            "start-in-background=false\nrun-on-startup=false\n"
        )
        subprocess.run(["glib-compile-schemas", "--strict", str(directory)], check=True)
    print(f"Prepared probes and memory-backend schemas in {build}")


if __name__ == "__main__":
    main()
