"""Protect account directories when Flatpak resets reserved XDG variables."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

from karere_probe import ROOT, isolated_command, verify_profile_processes


class ProfileLaunchTests(unittest.TestCase):
    """Exercise the actual guarded exec boundary without starting Karere."""

    def test_inside_sandbox_assignment_overrides_reset_environment(self):
        """The child receives private roots even if the outer sandbox resets them."""
        with tempfile.TemporaryDirectory(prefix="karere profile '") as directory:
            profile = Path(directory)
            environment = dict(os.environ, GSETTINGS_BACKEND='memory',
                XDG_DATA_HOME='/regular-app/data', XDG_CONFIG_HOME='/regular-app/config',
                XDG_CACHE_HOME='/regular-app/cache')
            command = ['/usr/bin/env', *isolated_command(profile, sys.executable), '-c',
                'import json,os,sys;print(json.dumps({k:os.environ[k] for k in '
                '["XDG_DATA_HOME","XDG_CONFIG_HOME","XDG_CACHE_HOME"]}));'
                'print(json.dumps(sys.argv[1:]))', 'literal $(never execute) with spaces']
            result = subprocess.run(command, env=environment, capture_output=True,
                                    text=True, check=True)
            lines = result.stdout.splitlines()
            self.assertEqual(lines[0], 'KARERE_PROFILE_ROOTS_VERIFIED')
            self.assertEqual(json.loads(lines[1]),
                {f'XDG_{name}_HOME': str(profile / name.lower())
                 for name in ['DATA', 'CONFIG', 'CACHE']})
            self.assertEqual(json.loads(lines[2]), ['literal $(never execute) with spaces'])

    def test_wrong_roots_and_persistent_settings_cannot_exec(self):
        """A bad environment is rejected before any application side effect."""
        for broken in ['XDG_DATA_HOME', 'XDG_CONFIG_HOME', 'XDG_CACHE_HOME', 'GSETTINGS_BACKEND']:
            with self.subTest(broken=broken), tempfile.TemporaryDirectory() as directory:
                profile = Path(directory)
                flag = profile / 'executed'
                environment = dict(os.environ, KARERE_PROBE_PROFILE_ROOT=str(profile),
                    GSETTINGS_BACKEND='memory', XDG_DATA_HOME=str(profile / 'data'),
                    XDG_CONFIG_HOME=str(profile / 'config'), XDG_CACHE_HOME=str(profile / 'cache'))
                environment[broken] = '/regular-app/data' if broken != 'GSETTINGS_BACKEND' else 'dconf'
                result = subprocess.run(['/bin/sh', str(ROOT / 'profile_exec.sh'),
                    sys.executable, '-c', 'from pathlib import Path;import sys;Path(sys.argv[1]).touch()',
                    str(flag)], env=environment, capture_output=True, text=True)
                self.assertEqual(result.returncode, 78)
                self.assertFalse(flag.exists())
                self.assertNotIn('KARERE_PROFILE_ROOTS_VERIFIED', result.stdout)

    def test_actual_process_and_cef_roots_are_checked(self):
        """Reject a normal CEF directory even if the process environment is private."""
        profile = Path('/tmp/karere-test-profile')
        environment = b'\0'.join(f'XDG_{name}_HOME={profile / name.lower()}'.encode()
                                  for name in ['DATA', 'CONFIG', 'CACHE'])
        for private in [True, False]:
            with self.subTest(private=private):
                cef = str(profile / 'data/karere/accounts/sessions') if private else '/regular-app/data/sessions'
                def read_bytes(path):
                    return environment if path.name == 'environ' else f'karere\0--user-data-dir={cef}\0'.encode()
                with patch('karere_probe.descendants', return_value=[123]), \
                     patch.object(Path, 'readlink', return_value=Path('/app/bin/karere')), \
                     patch.object(Path, 'read_bytes', read_bytes):
                    if private:
                        self.assertEqual(verify_profile_processes(123, profile),
                            {'main_processes_checked': 1, 'cef_directory_prefixes_checked': 1})
                    else:
                        with self.assertRaisesRegex(RuntimeError, 'non-diagnostic'):
                            verify_profile_processes(123, profile)


if __name__ == '__main__':
    unittest.main()
