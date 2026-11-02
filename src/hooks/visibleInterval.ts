/**
 * A `setInterval` that does no work while the window is hidden.
 *
 * Nectar is five windows that are almost always idle: the notch is a strip at
 * the top of the screen, the dock spends most of its life auto-hidden, and the
 * settings and calendar windows are usually closed. Every one of those windows
 * used to keep its pollers running regardless, so a hidden docking strip still
 * asked the host for media sessions and window lists several times a second.
 *
 * Chromium already throttles timers in hidden pages, but a throttled timer still
 * runs its callback eventually, and each callback here is an IPC round trip plus
 * a React state update. Skipping the callback outright removes that work entirely
 * and costs nothing: when the window becomes visible again the next tick (at
 * most one interval away) picks up current state, and every poller here is
 * idempotent and re-reads the truth from the host rather than accumulating.
 *
 * Usage is a drop-in replacement:
 *
 *   const id = setVisibleInterval(refresh, 2000);
 *   return () => clearInterval(id);
 */
export function setVisibleInterval(handler: () => void, ms: number): number {
  return window.setInterval(() => {
    if (document.hidden) return;
    handler();
  }, ms);
}

/**
 * True when the window is hidden or occluded. Exposed for one-shot effects that
 * should not start at all while the window is not on screen.
 */
export function isWindowHidden(): boolean {
  return document.hidden;
}
