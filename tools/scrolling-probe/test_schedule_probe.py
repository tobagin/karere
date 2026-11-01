"""Exercise ABI refusal without starting CEF or touching any profile."""
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parent

HEADER = r'''
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
struct handler {size_t size;void *base[4];void *other[4];void (*schedule)(struct handler *,int64_t);void *tail[2];};
struct app {size_t size;void *base[4];void *other[3];struct handler *(*get)(struct app *);void *tail;};
extern int observed_port, observed_tail;
int cef_initialize(const void *,const void *,void *,void *);
'''
LIBRARY = r'''
#include "fixture.h"
int observed_port, observed_tail;
#ifndef NO_VERSION
int cef_version_info(int entry) {(void)entry;return atoi(getenv("TEST_CEF_MAJOR"));}
#endif
int cef_initialize(const void *args,const void *settings,void *opaque,void *info) {
    (void)args;(void)info;
    const unsigned char *bytes=settings;
    memcpy(&observed_port,bytes+336,sizeof(observed_port));
    observed_tail=bytes[*(const size_t *)settings-1];
    struct app *app=opaque;struct handler *handler=app->get(app);
    handler->schedule(handler,7);
    return 1;
}
void cef_do_message_loop_work(void) {}
unsigned g_timeout_add_full(int priority,unsigned interval,void *function,void *data,void *notify) {
    (void)priority;(void)interval;(void)function;(void)data;(void)notify;return 1;
}
'''
PROGRAM = r'''
#include "fixture.h"
#include <stdio.h>
static int calls, delay;
static void schedule(struct handler *self,int64_t value) {(void)self;++calls;delay=(int)value;}
static struct handler handler={.size=sizeof(struct handler),.schedule=schedule};
static struct handler *get(struct app *app) {(void)app;return &handler;}
int main(int argc,char **argv) {
    (void)argc;
    union {max_align_t alignment;unsigned char bytes[512];} settings={0};
    size_t size=(size_t)atoi(argv[1]);memcpy(settings.bytes,&size,sizeof(size));
    int port=1234;memcpy(settings.bytes+336,&port,sizeof(port));settings.bytes[size-1]=165;
    struct app app={.size=sizeof(struct app),.get=get};
    int result=cef_initialize(NULL,settings.bytes,&app,NULL);
    memcpy(&port,settings.bytes+336,sizeof(port));
    printf("{\"result\":%d,\"port\":%d,\"original_port\":%d,\"tail\":%d,\"wrapped\":%d,\"calls\":%d,\"delay\":%d}\n",
        result,observed_port,port,observed_tail,app.get!=get&&handler.schedule!=schedule,calls,delay);
    return 0;
}
'''


@unittest.skipUnless(sys.platform == 'linux' and platform.machine() == 'x86_64'
                     and shutil.which('gcc'), 'Linux x86-64 and GCC required for interposer ABI tests')
class ScheduleABI(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix='karere-abi-test-')
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.directory = Path(cls.temporary.name)
        for name, content in [('fixture.h', HEADER), ('fixture.c', LIBRARY), ('main.c', PROGRAM)]:
            (cls.directory / name).write_text(content)
        def build(*arguments):
            subprocess.run(['gcc', '-Wall', '-Wextra', '-Werror', *arguments],
                           cwd=cls.directory, check=True, capture_output=True, text=True)
        build('-shared', '-fPIC', str(ROOT / 'schedule_probe.c'), '-ldl', '-o', 'probe.so')
        for name, extra in [('known', []), ('missing', ['-DNO_VERSION'])]:
            build('-shared', '-fPIC', *extra, 'fixture.c', '-o', f'lib{name}.so')
            build('main.c', '-L.', f'-l{name}', '-Wl,-rpath,$ORIGIN', '-o', name)

    def launch(self, major, size, name='known'):
        env = dict(os.environ, LD_PRELOAD=str(self.directory / 'probe.so'),
                   KARERE_PROBE_CDP='1', TEST_CEF_MAJOR=str(major))
        result = subprocess.run([str(self.directory / name), str(size)], env=env,
                                check=True, capture_output=True, text=True, timeout=5)
        return json.loads(result.stdout), result.stderr

    def test_verified_layouts_preserve_callbacks_and_settings(self):
        for major, size in [(152, 448), (156, 496)]:
            with self.subTest(major=major):
                row, log = self.launch(major, size)
                self.assertEqual(row, dict(result=1, port=9333, original_port=1234,
                                           tail=165, wrapped=1, calls=1, delay=7))
                self.assertIn('local_cdp_enabled', log)

    def test_unknown_or_mismatched_layouts_are_untouched(self):
        for major, size in [(160, 496), (156, 448), (152, 496), (156, 480)]:
            with self.subTest(major=major, size=size):
                row, log = self.launch(major, size)
                self.assertEqual(row, dict(result=1, port=1234, original_port=1234,
                                           tail=165, wrapped=0, calls=1, delay=7))
                self.assertIn('unsupported_cef_abi', log)

    def test_missing_version_symbol_is_untouched(self):
        row, log = self.launch(156, 496, 'missing')
        self.assertEqual(row['wrapped'], 0)
        self.assertEqual(row['port'], 1234)
        self.assertIn('major=0', log)


if __name__ == '__main__':
    unittest.main()
