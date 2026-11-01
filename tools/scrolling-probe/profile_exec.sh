#!/bin/sh
# Run only after env has applied the isolated roots inside the Flatpak sandbox.
set -eu
umask 077
if [ -z "${KARERE_PROBE_PROFILE_ROOT:-}" ] ||
   [ "${GSETTINGS_BACKEND:-}" != memory ] ||
   [ "${XDG_DATA_HOME:-}" != "$KARERE_PROBE_PROFILE_ROOT/data" ] ||
   [ "${XDG_CONFIG_HOME:-}" != "$KARERE_PROBE_PROFILE_ROOT/config" ] ||
   [ "${XDG_CACHE_HOME:-}" != "$KARERE_PROBE_PROFILE_ROOT/cache" ]; then
    printf '%s\n' 'Diagnostic profile roots do not match; application was not started.' >&2
    exit 78
fi
printf '%s\n' KARERE_PROFILE_ROOTS_VERIFIED
exec "$@"
