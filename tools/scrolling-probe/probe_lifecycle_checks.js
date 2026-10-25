/* Browser regression checks; validate_wallpaper.py refuses account pages. */
async (wallpaperSource, conversationSource) => {
  if (location.protocol !== 'data:' || !window.__generatedWallpaper) throw Error('Generated fixture required');
  if (window.__karereConversationProbe?.active || window.__karereWallpaperProbe) {
    throw Error('Finish or restore the existing experiment before validation');
  }
  const checks = [];
  const check = (name, passed) => checks.push({name, passed:!!passed});
  const throws = action => {try {action(); return false;} catch (_) {return true;}};
  const install = () => { (0, eval)(wallpaperSource); return window.__karereWallpaperProbe; };
  const hint = el => getComputedStyle(el).willChange;
  const main = document.querySelector('#main');
  const el = document.querySelector('#wallpaper');
  const node = document.getElementById('karere-conversation-wallpaper');
  const savedDisabled = node.sheet.disabled;
  const value = el.style.getPropertyValue('will-change'), priority = el.style.getPropertyPriority('will-change');
  try {
    for (const disabled of [false, true]) {
      node.sheet.disabled = disabled;
      el.style.removeProperty('will-change');
      const probe = install();
      check(`production mode detected, originally disabled=${disabled}`, probe.mode === 'production');
      for (const enabled of [false, true, true, false]) {
        probe.set(enabled);
        check(`production condition ${enabled}, originally disabled=${disabled}`,
          probe.verify(enabled) === (enabled ? 'transform' : 'auto'));
      }
      check('reinjection preserves the original snapshot', install() === probe);
      probe.restore();
      check(`production state restored, originally disabled=${disabled}`,
        node.sheet.disabled === disabled && el.style.getPropertyValue('will-change') === '');
    }
    node.sheet.disabled = false;
    el.style.setProperty('will-change', 'opacity', 'important');
    let probe = install();
    check('conflicting production inline hint rejects off', throws(() => probe.set(false)));
    probe.restore();
    check('conflicting inline value and priority restored', !node.sheet.disabled &&
      el.style.getPropertyValue('will-change') === 'opacity' &&
      el.style.getPropertyPriority('will-change') === 'important');
    el.style.removeProperty('will-change');

    node.remove();
    el.style.setProperty('will-change', 'auto', 'important');
    probe = install();
    check('missing production stylesheet selects inline mode', probe.mode === 'inline');
    probe.set(true);
    check('legacy inline hint turns on', hint(el) === 'transform');
    probe.set(false);
    check('legacy inline hint turns off', hint(el) === 'auto');
    probe.restore();
    check('legacy inline priority restored', el.style.getPropertyValue('will-change') === 'auto' &&
      el.style.getPropertyPriority('will-change') === 'important');
    el.style.removeProperty('will-change');
    document.head.appendChild(node);

    probe = install();
    probe.set(false);
    const replacement = el.cloneNode(false);
    el.replaceWith(replacement);
    try {
      check('replaced wallpaper invalidates condition', throws(() => probe.verify(false)));
      check('reinjection keeps disconnected wallpaper cleanup state', install() === probe);
      probe.restore();
      check('stylesheet restored after wallpaper replacement', !node.sheet.disabled);
    } finally {replacement.replaceWith(el);}

    probe = install();
    probe.set(false);
    const nextStyle = node.cloneNode(true);
    node.replaceWith(nextStyle);
    try {
      check('replaced stylesheet invalidates condition', throws(() => probe.verify(false)));
      probe.restore();
    } finally {nextStyle.replaceWith(node);}
    probe = install();
    check('wrong wallpaper session rejected', throws(() => probe.verify(true, 'different-session')));
    probe.restore();
  } finally {
    window.__karereWallpaperProbe?.restore();
    if (!node.isConnected) document.head.appendChild(node);
    node.sheet.disabled = savedDisabled;
    if (value) el.style.setProperty('will-change', value, priority);
    else el.style.removeProperty('will-change');
  }

  (0, eval)(conversationSource);
  const probe = window.__karereConversationProbe;
  const pane = main.querySelector('#conversation'), original = pane.scrollTop;
  try {
    probe.start({runId:'early-cancel', warmup:0});
    check('wrong owner cannot cancel sampling', probe.cancel('different-owner') === false && probe.active);
    (0, eval)(conversationSource);
    check('reinjection preserves active sampling', window.__karereConversationProbe === probe && probe.active);
    check('second start rejected without replacing owner', throws(() => probe.start({runId:'other'})) &&
      probe.runId === 'early-cancel');
    probe.cancel('early-cancel');
    const result = await probe.done;
    check('early cancellation produces no valid measurement', result.status === 'cancelled' && result.observed_frames === 0);
    check('early cancellation keeps finite timestamps', Number.isFinite(result.start_epoch_ms) &&
      result.end_epoch_ms >= result.start_epoch_ms && result.scroll_height_changes === 0);
    check('early cancellation restores scroll position', pane.scrollTop === original && !probe.active);
    probe.cancel('early-cancel');
    check('cancellation is idempotent', probe.result === result && pane.scrollTop === original);

    const observe = PerformanceObserver.prototype.observe;
    PerformanceObserver.prototype.observe = () => {throw Error('Synthetic setup failure');};
    try {
      check('setup failure restores position and stops probe', throws(() => probe.start({runId:'setup-failure'})) &&
        !probe.active && pane.scrollTop === original && probe.result.status === 'cancelled');
    } finally {PerformanceObserver.prototype.observe = observe;}

    probe.start({runId:'manual-finish', warmup:0, mode:'observe', duration:5000});
    await new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
    check('wrong owner cannot finish manual capture', probe.finish('other') === false && probe.active);
    const manual = probe.finish('manual-finish');
    check('manual finish keeps a valid owned sample', manual.status === 'complete' && manual.observed_frames > 0);
    probe.cancel('manual-finish');
    check('cleanup preserves completed results', probe.result === manual && manual.status === 'complete' &&
      pane.scrollTop === original && !probe.active);

    probe.start({runId:'pane-replaced', warmup:0});
    const replacement = pane.cloneNode(true);
    pane.replaceWith(replacement);
    try {
      await probe.done;
      check('replaced scroller aborts the measurement', probe.result.status === 'cancelled' && !probe.active);
    } finally {replacement.replaceWith(pane);}
  } finally {
    probe.cancel(probe.runId);
    pane.scrollTo({top:original, behavior:'instant'});
  }
  return checks;
}
