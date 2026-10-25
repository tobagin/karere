/* Reversible experiment: isolate the large decorative mask without changing it. */
(() => {
  if (window.__karereWallpaperProbe) {
    if (window.__karereWallpaperProbe.version !== 2) throw Error('Reload the old diagnostic probe first');
    // Never discard the saved stylesheet/inline state after an element replacement.
    return true;
  }
  const main = document.querySelector('#main');
  if (!main) throw Error('Conversation unavailable');
  const m = main.getBoundingClientRect();
  const candidates = [...document.querySelectorAll('div')].filter(e => {
    const r = e.getBoundingClientRect(), s = getComputedStyle(e);
    return Math.abs(r.x - m.x) < 3 && r.width >= m.width * .95 && r.height >= m.height * .95 &&
      s.position === 'absolute' && s.maskImage !== 'none';
  });
  if (candidates.length !== 1) throw Error('Decorative mask is not uniquely identified');
  const el = candidates[0], style = getComputedStyle(el), r = el.getBoundingClientRect();
  const node = document.getElementById('karere-conversation-wallpaper');
  if (node && !node.sheet) throw Error('Production stylesheet unavailable');
  const sheet = node?.sheet || null;
  window.__karereWallpaperProbe = {
    version:2, el, mode:sheet ? 'production' : 'inline',
    id:String(performance.timeOrigin) + ':' + String(performance.now()),
    value:el.style.getPropertyValue('will-change'), priority:el.style.getPropertyPriority('will-change'),
    disabled:sheet?.disabled,
    intact() {
      if (!this.el.isConnected || document.querySelector('#main') !== main ||
          document.getElementById('karere-conversation-wallpaper') !== node ||
          (node && node.sheet !== sheet)) throw Error('Wallpaper element or stylesheet replaced');
    },
    verify(enabled, id=this.id) {
      this.intact();
      if (id !== this.id) throw Error('Wallpaper experiment replaced');
      const actual = getComputedStyle(this.el).willChange;
      if (actual !== (enabled ? 'transform' : 'auto')) throw Error('Wallpaper condition did not switch as expected');
      return actual;
    },
    set(enabled) {
      this.intact();
      if (sheet) sheet.disabled = !enabled;
      else if (enabled) this.el.style.setProperty('will-change', 'transform', 'important');
      else if (this.value) this.el.style.setProperty('will-change', this.value, this.priority);
      else this.el.style.removeProperty('will-change');
      return this.verify(enabled);
    },
    restore() {
      if (this.value) this.el.style.setProperty('will-change', this.value, this.priority);
      else this.el.style.removeProperty('will-change');
      if (sheet) sheet.disabled = this.disabled;
      if (window.__karereWallpaperProbe === this) delete window.__karereWallpaperProbe;
      return true;
    },
    geometry:{x:r.x, y:r.y, w:r.width, h:r.height, mask_is_svg:style.maskImage.includes('svg'),
      mask_repeat:style.maskRepeat, mask_size:style.maskSize, pointer_events:style.pointerEvents},
  };
  return true;
})()
