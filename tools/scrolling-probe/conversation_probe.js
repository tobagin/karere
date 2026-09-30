/* Timing and geometry only. No message text, account data, clicks or keystrokes. */
(() => {
  if (window.__karereConversationProbe?.version === 2) return true;
  if (window.__karereConversationProbe?.active) throw Error('Probe already active');
  const quantiles = values => {
    const a = [...values].sort((a, b) => a - b);
    const q = p => a.length ? a[Math.floor((a.length - 1) * p)] : 0;
    return {n:a.length, p50:q(.5), p95:q(.95), p99:q(.99), max:q(1)};
  };
  const choose = target => {
    const root = document.querySelector(target === 'list' ? '#pane-side' : '#main');
    if (!root) throw Error('Requested pane unavailable');
    const candidates = [root, ...root.querySelectorAll('*')].filter(e =>
      e.clientHeight > 100 && e.clientWidth > 100 && e.scrollHeight > e.clientHeight + 300 &&
      /auto|scroll/.test(getComputedStyle(e).overflowY));
    candidates.sort((a, b) => b.clientWidth * b.clientHeight - a.clientWidth * a.clientHeight);
    if (!candidates.length) throw Error('Scrollable pane unavailable');
    return candidates[0];
  };
  window.__karereConversationProbe = {
    version:2, active:false, result:null, runId:null,
    finish(runId) {
      if (runId !== this.runId) return false;
      if (this.active) this.stop('complete');
      return this.result;
    },
    cancel(runId) {
      if (runId !== this.runId) return false;
      if (this.active) this.stop('cancelled');
      return true;
    },
    start({runId, target='conversation', mode='programmatic', warmup=5000,
           duration=20000, amplitude=900, anchorBottom=null}={}) {
      if (this.active) throw Error('Probe already active');
      if (typeof runId !== 'string' || !runId) throw Error('Run identity required');
      const pane = choose(target), original = pane.scrollTop, maximum = pane.scrollHeight - pane.clientHeight;
      if (maximum < amplitude + 400) throw Error('Insufficient loaded scroll range');
      const anchor = anchorBottom === null ? original : maximum - anchorBottom;
      if (anchorBottom !== null && anchor < amplitude + 400) throw Error('Load more history before matching the anchor');
      const base = Math.max(200, Math.min(anchor - amplitude - 200, maximum - amplitude - 200));
      const rect = pane.getBoundingClientRect();
      const geometry = {x:rect.x, y:rect.y, width:rect.width, height:rect.height,
        client_width:pane.clientWidth, client_height:pane.clientHeight, scroll_height:pane.scrollHeight,
        viewport_width:innerWidth, viewport_height:innerHeight, dpr:devicePixelRatio};
      let started, last, frameId, resolveDone;
      const frames = [], wheels = [], scrolls = [], longTasks = [];
      const wheel = e => wheels.push([performance.now(), e.deltaX, e.deltaY, e.deltaMode, e.isTrusted]);
      const scroll = () => scrolls.push([performance.now(), pane.scrollTop]);
      const observer = new PerformanceObserver(list => {
        for (const e of list.getEntries()) longTasks.push([e.startTime, e.duration]);
      });
      this.active = true;
      this.runId = runId;
      this.result = null;
      this.done = new Promise(resolve => {resolveDone = resolve;});
      this.stop = status => {
        if (!this.active) return;
        cancelAnimationFrame(frameId);
        observer.disconnect();
        pane.removeEventListener('wheel', wheel);
        pane.removeEventListener('scroll', scroll);
        const now = performance.now();
        const begin = started === undefined ? now : Math.min(started + warmup, now);
        const end = Math.max(begin, Math.min(begin + duration, now));
        const activeFrames = frames.filter(f => f[0] >= begin && f[0] < end);
        const intervals = activeFrames.map(f => f[1]).filter(x => x !== null);
        const steps = activeFrames.slice(1).map((f, i) => Math.abs(f[2] - activeFrames[i][2]));
        const moved = steps.filter(x => x > .1);
        if (status === 'complete' && (!activeFrames.length || end <= begin || !pane.isConnected)) {
          status = 'incomplete';
        }
        const result = {run_id:runId, status, target, mode, geometry, base, amplitude,
          anchor_bottom:anchorBottom, original_top:original,
          start_epoch_ms:performance.timeOrigin + begin, end_epoch_ms:performance.timeOrigin + end,
          time_origin_ms:performance.timeOrigin, frame_intervals_ms:quantiles(intervals),
          gaps_over_25ms:intervals.filter(x => x > 25).length,
          gaps_over_33ms:intervals.filter(x => x > 33.4).length,
          scroll_steps_px:quantiles(moved), moving_frames:moved.length, observed_frames:activeFrames.length,
          scroll_height_changes:Math.max(0, new Set(activeFrames.map(f => f[3])).size - 1),
          long_tasks:longTasks.filter(t => t[0] >= begin && t[0] < end),
          frames:activeFrames, wheels:wheels.filter(e => e[0] >= begin && e[0] < end),
          scrolls:scrolls.filter(e => e[0] >= begin && e[0] < end), visibility:document.visibilityState};
        pane.scrollTo({top:original, behavior:'instant'});
        this.active = false;
        this.result = result;
        resolveDone(result);
      };
      const frame = t => {
        if (!pane.isConnected) {this.stop('cancelled'); return;}
        if (started === undefined) started = t;
        const age = t - started;
        if (age >= warmup + duration) {this.stop('complete'); return;}
        if (mode === 'programmatic') {
          const phase = (age / 3000) % 2;
          pane.scrollTo({top:base + (phase <= 1 ? phase : 2 - phase) * amplitude, behavior:'instant'});
        }
        frames.push([t, last === undefined ? null : t - last, pane.scrollTop, pane.scrollHeight]);
        last = t;
        frameId = requestAnimationFrame(frame);
      };
      try {
        pane.scrollTo({top:base, behavior:'instant'});
        const testTop = pane.scrollTop;
        pane.scrollTo({top:base + 2, behavior:'instant'});
        if (Math.abs(pane.scrollTop - testTop) < 1) throw Error('Pane did not move');
        pane.scrollTo({top:base, behavior:'instant'});
        pane.addEventListener('wheel', wheel, {passive:true});
        pane.addEventListener('scroll', scroll, {passive:true});
        observer.observe({type:'longtask'});
        frameId = requestAnimationFrame(frame);
      } catch (error) {
        this.stop('cancelled');
        throw error;
      }
      return geometry;
    }
  };
  return true;
})()
