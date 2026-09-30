"""Target selection and bounded cleanup shared by conversation diagnostics."""
import json
import signal
import subprocess
import sys
import urllib.parse
import urllib.request
from contextlib import contextmanager

from cdp_local import Client


def connect(target_id=None):
    """Connect to an explicit visible eligible target, or the only such target."""
    with urllib.request.urlopen('http://127.0.0.1:9333/json/list', timeout=3) as response:
        targets = json.load(response)
    visible = []
    try:
        for target in targets:
            url = target.get('url', '')
            eligible = target.get('type') == 'page' and (
                urllib.parse.urlparse(url).hostname == 'web.whatsapp.com' or url.startswith('data:'))
            if not eligible or (target_id is not None and target.get('id') != target_id):
                continue
            client = Client(target['webSocketDebuggerUrl'])
            try:
                if client.evaluate("document.visibilityState === 'visible'"):
                    client.target_id = target['id']
                    visible.append(client)
                    client = None
            finally:
                if client is not None:
                    client.close()
        if len(visible) != 1:
            if not visible:
                raise RuntimeError('Requested visible diagnostic target unavailable')
            raise RuntimeError('Multiple visible diagnostic targets; choose --target-id: ' +
                               ', '.join(client.target_id for client in visible))
        return visible.pop()
    finally:
        for client in visible:
            client.close()


def target_argument(parser):
    """Expose explicit page selection without exposing page titles or URLs."""
    parser.add_argument('--target-id', help='CDP page ID; required when multiple eligible pages are visible')


def validate_label(parser, label):
    """Keep result filenames confined to the diagnostic results directory."""
    if not label or not label.isascii() or not label.replace('_', '').replace('-', '').isalnum():
        parser.error('Label must contain only ASCII letters, digits, underscores or hyphens')


def cleanup(action, description):
    """Attempt cleanup, preserving an existing failure rather than masking it."""
    failing = sys.exc_info()[0] is not None
    try:
        return action()
    except Exception as error:
        if not failing:
            raise
        print(f'{description} failed ({type(error).__name__}); close the diagnostic instance.', file=sys.stderr)
        return None


def cancel_probe(client, run_id):
    """Cancel only the named run, leaving another operator's probe untouched."""
    return client.evaluate('window.__karereConversationProbe?.cancel(' + json.dumps(run_id) + ') ?? false')


def stop_child(child):
    """Give an interrupted child time to clean up, then kill and reap if needed."""
    if child.poll() is None:
        child.terminate()
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            child.kill()
            child.wait(timeout=5)


def run_probe_child(command, client, run_id):
    """Pin a child to the parent's target and cancel its owned run on every exit."""
    child = None
    try:
        child = subprocess.Popen(command + ['--target-id', client.target_id, '--run-id', run_id],
                                 start_new_session=True)
        code = child.wait()
        if code:
            raise subprocess.CalledProcessError(code, command)
    finally:
        try:
            if child is not None:
                cleanup(lambda: stop_child(child), 'Child shutdown')
        finally:
            cleanup(lambda: cancel_probe(client, run_id), 'Page cancellation')


@contextmanager
def interrupt_cleanup():
    """Let SIGTERM follow the same finally-based cleanup path as Ctrl-C."""
    def interrupt(_signum, _frame):
        """Convert process termination into a cleanup-capable interruption."""
        raise KeyboardInterrupt

    previous = signal.signal(signal.SIGTERM, interrupt)
    try:
        yield
    finally:
        signal.signal(signal.SIGTERM, previous)
