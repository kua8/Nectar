export function setVisibleInterval(handler: () => void, ms: number): number {
  return window.setInterval(() => {
    if (document.hidden) return;
    handler();
  }, ms);
}
