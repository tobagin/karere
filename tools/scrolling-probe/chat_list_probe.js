/* Scroll geometry and timing only: never reads messages, contacts or account data.
 * Restores the starting scroll offset. No clicks, keyboard input or message sends. */
(async () => {
  const root = document.querySelector('#pane-side');
  if (!root) return {ready: false};
  const pane = [root, ...root.querySelectorAll('*')].find(element =>
    element.clientHeight > 100 && element.scrollHeight > element.clientHeight + 200 &&
    /auto|scroll/.test(getComputedStyle(element).overflowY));
  if (!pane) return {ready: false};
  const originalTop = pane.scrollTop;
  const amplitude = Math.min(900, pane.scrollHeight - pane.clientHeight);
  const base = Math.min(originalTop, pane.scrollHeight - pane.clientHeight - amplitude);
  const intervals = [], longTasks = [];
  const observer = new PerformanceObserver(list => {
    for (const item of list.getEntries()) longTasks.push({start: item.startTime, duration: item.duration});
  });
  observer.observe({type: 'longtask'});
  let started, last, changes = 0, moved = 0, previousTop = pane.scrollTop;
  const duration = 15000, warmup = 5000;
  return await new Promise(resolve => {
    function frame(t) {
      if (started === undefined) started = t;
      const age = t - started;
      if (age >= warmup && age < warmup + duration) {
        if (last !== undefined) intervals.push(t - last);
        last = t;
        const leg = ((age - warmup) / 3000) % 2;
        pane.scrollTop = base + (leg <= 1 ? leg : 2 - leg) * amplitude;
        if (Math.abs(pane.scrollTop - previousTop) > 0.1) moved++;
        previousTop = pane.scrollTop;
        changes++;
      }
      if (age < warmup + duration) { requestAnimationFrame(frame); return; }
      observer.disconnect();
      pane.scrollTop = originalTop;
      const sorted = [...intervals].sort((a, b) => a - b);
      const q = p => sorted[Math.floor((sorted.length - 1) * p)] || 0;
      const activeTasks = longTasks.filter(item => item.start >= started + warmup && item.start < started + warmup + duration);
      resolve({ready: true, start_epoch_ms: performance.timeOrigin + started + warmup,
        end_epoch_ms: performance.timeOrigin + started + warmup + duration,
        duration_ms: duration, raf_frames: changes, raf_median_ms: q(.5),
        raf_p95_ms: q(.95), raf_max_ms: Math.max(...intervals, 0),
        raf_gaps_over_33ms: intervals.filter(x => x > 33.4).length,
        inner_width: innerWidth, inner_height: innerHeight, device_pixel_ratio: devicePixelRatio,
        visibility: document.visibilityState, scroll_changes: moved, amplitude_px: amplitude,
        long_tasks: activeTasks.length, long_task_total_ms: activeTasks.reduce((sum, item) => sum + item.duration, 0)});
    }
    requestAnimationFrame(frame);
  });
})()
