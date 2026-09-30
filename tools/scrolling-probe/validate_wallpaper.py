#!/usr/bin/env python3
"""Evaluate the production wallpaper script in CEF's isolated generated fixture."""
import argparse
import json
from pathlib import Path

from probe_common import connect, interrupt_cleanup, target_argument, validate_label

ROOT = Path(__file__).resolve().parent

CHECKS = r"""(() => {
  const original = document.querySelector('#main');
  const originalWallpaper = original.querySelector('#wallpaper');
  const style = document.getElementById('karere-conversation-wallpaper');
  if (!originalWallpaper || !style?.sheet) throw Error('Generated fixture or stylesheet unavailable');
  const checks = [];
  const check = (name, passed) => checks.push({name, passed});
  const hint = el => getComputedStyle(el).willChange;
  const mask = originalWallpaper.style.maskImage;
  check('installed once', !!style &&
    document.querySelectorAll('#karere-conversation-wallpaper').length === 1);
  check('generated decorative wallpaper is promoted', hint(originalWallpaper) === 'transform');
  check('scroll container is untouched', hint(original.querySelector('#conversation')) === 'auto');
  check('message content is untouched', hint(original.querySelector('.bubble')) === 'auto');

  const replacement = document.createElement('div');
  replacement.id = 'main';
  replacement.style.cssText = 'position:relative;left:0;width:600px;height:500px';
  const background = document.createElement('div');
  background.style.cssText = 'position:absolute;inset:0;opacity:.6;background:#fff';
  background.style.maskImage = mask;
  const messages = document.createElement('div');
  messages.textContent = 'Generated message content';
  messages.style.maskImage = mask;
  const nestedMask = document.createElement('div');
  nestedMask.style.maskImage = mask;
  messages.appendChild(nestedMask);
  replacement.append(background, messages);
  original.replaceWith(replacement);
  try {
    check('replacement chat receives the hint automatically', hint(background) === 'transform');
    check('nonempty direct child is untouched', hint(messages) === 'auto');
    check('nested masked media is untouched', hint(nestedMask) === 'auto');
    const before = background.getBoundingClientRect();
    style.sheet.disabled = true;
    const without = background.getBoundingClientRect();
    check('hint preserves background geometry', ['x','y','width','height'].every(k => before[k] === without[k]));
    style.sheet.disabled = false;
    check('hint preserves the original mask', getComputedStyle(background).maskImage === mask);
    background.style.backgroundColor = '#111b21';
    check('background color changes retain the hint', hint(background) === 'transform');
    background.style.maskImage = 'none';
    check('unmasked custom background is not promoted', hint(background) === 'auto');
    background.style.removeProperty('mask-image');
    check('removed mask is not promoted', hint(background) === 'auto');
    background.style.maskImage = mask;
    replacement.style.width = '280px';
    check('narrow layout keeps the mask and correct width', hint(background) === 'transform' &&
      background.getBoundingClientRect().width === 280);
    const next = background.cloneNode(false);
    background.replaceWith(next);
    check('replacement background receives the hint automatically', hint(next) === 'transform');
    replacement.id = 'not-a-conversation';
    check('mask outside the conversation is untouched', hint(next) === 'auto');
  } finally {
    style.sheet.disabled = false;
    replacement.replaceWith(original);
  }
  return checks;
})()"""


def main():
    """Validate production scope and probe recovery only on the generated fixture."""
    parser = argparse.ArgumentParser()
    parser.add_argument('--label', default='wallpaper_scope_checks', help='Fresh result label')
    target_argument(parser)
    args = parser.parse_args()
    validate_label(parser, args.label)
    output = ROOT / 'results' / f'{args.label}.json'
    output.parent.mkdir(exist_ok=True)
    if output.exists():
        parser.error('Validation output already exists; choose a fresh --label')
    client = connect(args.target_id)
    try:
        if not client.evaluate("location.protocol === 'data:' && !!window.__generatedWallpaper"):
            raise SystemExit('Refusing to modify a real page; launch conversation_probe.html with --synthetic')
        if client.evaluate('!!window.__karereConversationProbe?.active || !!window.__karereWallpaperProbe'):
            raise SystemExit('Finish or restore the existing experiment before validation')
        source = (ROOT.parents[1] / 'data/js/80-conversation-wallpaper.js').read_text()
        # The existing identity hook reads localStorage synchronously, which
        # throws on an opaque data: origin before later bundle scripts execute.
        # Evaluate this production component directly here. Automatic bundle
        # installation is verified separately on the real WhatsApp page.
        client.evaluate(source)
        results = client.evaluate(CHECKS)
        # Execute the actual production script again, rather than a mock DOM.
        client.evaluate(source)
        results.append({'name': 'reinjection remains idempotent', 'passed': client.evaluate(
            "document.querySelectorAll('#karere-conversation-wallpaper').length === 1")})
        lifecycle = (ROOT / 'probe_lifecycle_checks.js').read_text()
        wallpaper = (ROOT / 'wallpaper_probe.js').read_text()
        conversation = (ROOT / 'conversation_probe.js').read_text()
        results.extend(client.evaluate('(' + lifecycle + ')(' + json.dumps(wallpaper) + ',' +
                                       json.dumps(conversation) + ')', await_promise=True))
        with output.open('x') as stream:
            json.dump(results, stream, indent=2)
            stream.write('\n')
        print(json.dumps(results))
        if not all(check['passed'] for check in results):
            raise SystemExit('Wallpaper scope validation failed')
    finally:
        client.close()


if __name__ == '__main__':
    with interrupt_cleanup():
        main()
