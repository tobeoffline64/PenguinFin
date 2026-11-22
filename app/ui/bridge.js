// Browser-side helpers for PenguinFin 2, the same behaviour as 1.x:
// - Ctrl/Cmd + / - / 0 dispatch "pf-scale" ("bigger", "smaller", "auto") for the interface.
// - When window.__pfNatural is true, wheel and touchpad scrolling is reversed ("natural
//   scrolling"), applied to the nearest element that can scroll that way.
(() => {
  if (window.__pfBridge) return;
  window.__pfBridge = true;
  window.addEventListener('keydown', (e) => {
    if (!(e.ctrlKey || e.metaKey) || e.altKey) return;
    let cmd = null;
    if (e.key === '=' || e.key === '+') cmd = 'bigger';
    else if (e.key === '-' || e.key === '_') cmd = 'smaller';
    else if (e.key === '0') cmd = 'auto';
    if (cmd) { e.preventDefault(); window.dispatchEvent(new CustomEvent('pf-scale', { detail: cmd })); }
  });
  const canScroll = (el, dx, dy) => {
    const s = getComputedStyle(el);
    const y = /(auto|scroll)/.test(s.overflowY) && el.scrollHeight > el.clientHeight + 1 &&
      ((dy < 0 && el.scrollTop > 0) || (dy > 0 && el.scrollTop + el.clientHeight < el.scrollHeight - 1));
    const x = /(auto|scroll)/.test(s.overflowX) && el.scrollWidth > el.clientWidth + 1 &&
      ((dx < 0 && el.scrollLeft > 0) || (dx > 0 && el.scrollLeft + el.clientWidth < el.scrollWidth - 1));
    return x || y;
  };
  window.addEventListener('wheel', (e) => {
    if (!window.__pfNatural || e.ctrlKey) return;
    const unit = e.deltaMode === 1 ? 16 : (e.deltaMode === 2 ? window.innerHeight : 1);
    const dx = -e.deltaX * unit, dy = -e.deltaY * unit;
    let el = e.target instanceof Element ? e.target : null;
    while (el && el !== document.documentElement && !canScroll(el, dx, dy)) el = el.parentElement;
    if (!el || el === document.documentElement) el = document.scrollingElement;
    e.preventDefault();
    el.scrollBy({ left: dx, top: dy, behavior: e.deltaMode === 0 && Math.abs(e.deltaY) < 40 ? 'auto' : 'smooth' });
  }, { passive: false, capture: true });
})();
