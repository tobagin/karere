#!/usr/bin/env bash
# Container entrypoint: resolve the CEF branch, build each arch via
# build-cef-codecs.sh, copy the verified archive + checksums/provenance to /out.
set -euo pipefail

export CEF_BUILD_MODE="${CEF_BUILD_MODE:-upgrade}"
if [[ "$CEF_BUILD_MODE" == legacy-current-pin ]]; then
  export CEF_BRANCH="${CEF_BRANCH:-7977}"
  export CEF_CHECKOUT="${CEF_CHECKOUT:-708dc140cbc3286826a8abef89dc23a44ff9ea72}"
  export CEF_VERSION="${CEF_VERSION:-152.0.6+g708dc14+chromium-152.0.7977.83}"
fi

# Keep /work on a persistent volume; Chromium history is unnecessary, but CEF
# history is needed for the generated version header.
export AUTOMATE_EXTRA="${AUTOMATE_EXTRA:---no-chromium-history}"

for arch in ${ARCHES:-x64 arm64}; do
  echo ">> ===== building $arch ====="
  CEF_ARCH=$arch /opt/karere/build-cef-codecs.sh /work
  tarball=$(python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["artifact"])' "/work/cef-artifact-$arch.json")
  mkdir -p /out
  cp "$tarball" "$tarball.sha256" "$tarball.provenance.json" /out/
  (cd /out && sha256sum --check "$(basename "$tarball").sha256")
done

echo ">> done — results in /out:"
ls -l /out
cat /out/*.sha256
