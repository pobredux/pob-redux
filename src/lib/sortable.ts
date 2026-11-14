export interface SortZone<C extends string> {
  column: C;
  /** Must be positioned: its `[data-sort]` children are measured with offsetTop. */
  el: HTMLElement | undefined;
  /** Items flow in rows and columns: the dragged one takes the place of the item under the pointer. */
  grid?: boolean;
}

interface SortOptions<C extends string> {
  id: string;
  zones: () => SortZone<C>[];
  scroller: () => HTMLElement | undefined;
  move: (column: C, index: number) => void;
  onstart: () => void;
  onend: () => void;
}

const THRESHOLD = 4;
const EDGE = 48;
const LIFT = 1.015;
const SETTLE = "transform 220ms cubic-bezier(.2, .8, .2, 1), opacity 180ms ease, box-shadow 180ms ease";

function swallow(e: Event) {
  e.stopPropagation();
  e.preventDefault();
}

function findItem<C extends string>(zones: SortZone<C>[], id: string): { el: HTMLElement; zone: HTMLElement } | null {
  for (const zone of zones) {
    const el = zone.el?.querySelector<HTMLElement>(`:scope > [data-sort="${CSS.escape(id)}"]`);
    if (el && zone.el) return { el, zone: zone.el };
  }
  return null;
}

/** Where an item's slot is on screen, without the transform that carries it under the pointer. */
function slot(el: HTMLElement, zone: HTMLElement) {
  const r = zone.getBoundingClientRect();
  return { left: r.left + zone.clientLeft - zone.scrollLeft + el.offsetLeft, top: r.top + zone.clientTop - zone.scrollTop + el.offsetTop, width: el.offsetWidth };
}

function dropTarget<C extends string>(id: string, zones: SortZone<C>[], x: number, y: number): { column: C; index: number } | null {
  let best: { zone: SortZone<C>; el: HTMLElement; d: number } | null = null;
  for (const zone of zones) {
    if (!zone.el) continue;
    const r = zone.el.getBoundingClientRect();
    const d = Math.hypot(Math.max(r.left - x, 0, x - r.right), Math.max(r.top - y, 0, y - r.bottom));
    if (!best || d < best.d) best = { zone, el: zone.el, d };
  }
  if (!best) return null;
  const { el } = best;
  const box = el.getBoundingClientRect();
  const left = box.left + el.clientLeft - el.scrollLeft;
  const top = box.top + el.clientTop - el.scrollTop;
  const items = [...el.querySelectorAll<HTMLElement>(":scope > [data-sort]")];
  // offsetTop ignores the flip animation's transforms, so the target does not flicker mid-move.
  if (best.zone.grid) {
    const under = (item: HTMLElement) =>
      x >= left + item.offsetLeft && x < left + item.offsetLeft + item.offsetWidth && y >= top + item.offsetTop && y < top + item.offsetTop + item.offsetHeight;
    const self = items.findIndex((item) => item.dataset.sort === id);
    const over = items.findIndex(under);
    if (over < 0 || over === self) return null;
    const others = items.filter((item) => item.dataset.sort !== id);
    const at = others.indexOf(items[over]);
    return { column: best.zone.column, index: self >= 0 && over > self ? at + 1 : at };
  }
  let index = 0;
  for (const item of items) {
    if (item.dataset.sort !== id && top + item.offsetTop + item.offsetHeight / 2 < y) index++;
  }
  return { column: best.zone.column, index };
}

/** Starts a drag from a pointerdown; it becomes a drag only after the pointer moves a few pixels. */
export function sortDrag<C extends string>(e: PointerEvent, o: SortOptions<C>) {
  if (e.button !== 0) return;
  const sx = e.clientX;
  const sy = e.clientY;
  let x = sx;
  let y = sy;
  let active = false;
  let frame = 0;
  // Where the item was grabbed: a share of its width, since a move to another column can change the width.
  let grab = { rx: 0, dy: 0 };
  const place = () => {
    const t = dropTarget(o.id, o.zones(), x, y);
    if (t) o.move(t.column, t.index);
  };
  // Runs every frame: the item may have been remounted or re-laid out since the last one.
  const follow = () => {
    const found = findItem(o.zones(), o.id);
    if (!found) return;
    const s = slot(found.el, found.zone);
    found.el.style.transform = `translate(${x - grab.rx * s.width - s.left}px, ${y - grab.dy - s.top}px) scale(${LIFT})`;
  };
  const tick = () => {
    const el = o.scroller();
    const r = el?.getBoundingClientRect();
    const dy = !r ? 0 : y < r.top + EDGE ? y - r.top - EDGE : y > r.bottom - EDGE ? y - r.bottom + EDGE : 0;
    if (el && dy) {
      el.scrollTop += dy / 4;
      place();
    }
    follow();
    frame = requestAnimationFrame(tick);
  };
  const onmove = (ev: PointerEvent) => {
    x = ev.clientX;
    y = ev.clientY;
    if (!active) {
      if (Math.hypot(x - sx, y - sy) < THRESHOLD) return;
      active = true;
      const found = findItem(o.zones(), o.id);
      if (found) {
        const r = found.el.getBoundingClientRect();
        grab = { rx: (sx - r.left) / r.width, dy: sy - r.top };
        found.el.style.transition = "opacity 180ms ease, box-shadow 180ms ease";
      }
      o.onstart();
      frame = requestAnimationFrame(tick);
    }
    place();
  };
  // Window listeners: an item moved to another zone remounts, which would drop a pointer capture.
  const end = () => {
    window.removeEventListener("pointermove", onmove);
    window.removeEventListener("pointerup", end);
    window.removeEventListener("pointercancel", end);
    if (!active) return;
    cancelAnimationFrame(frame);
    window.addEventListener("click", swallow, { capture: true, once: true });
    setTimeout(() => window.removeEventListener("click", swallow, true), 0);
    const found = findItem(o.zones(), o.id);
    if (found) {
      const el = found.el;
      el.style.transition = SETTLE;
      el.style.transform = "";
      setTimeout(() => {
        if (!el.style.transform) el.style.transition = "";
      }, 260);
    }
    o.onend();
  };
  window.addEventListener("pointermove", onmove);
  window.addEventListener("pointerup", end);
  window.addEventListener("pointercancel", end);
}
