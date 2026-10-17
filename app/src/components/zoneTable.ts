// Tables of steps grouped by zone, their zone lines stuck at the top (the guide editor).
import { useEffect, type RefObject } from "react";

/**
 * Before closing a zone from its line stuck at the top (its steps scrolled under it): once
 * closed, its line is brought back at the top instead of leaving the view further down.
 */
export function keepZoneInView(row: HTMLElement) {
  const scroller = scrollParent(row);
  const sticky = parseFloat(getComputedStyle(row).scrollMarginTop) || 0;
  const top = (scroller?.getBoundingClientRect().top ?? 0) + sticky;
  if (row.getBoundingClientRect().top <= top + 1) requestAnimationFrame(() => row.scrollIntoView({ block: "start" }));
}

/** After closing every zone: back to the top of the table when it is above the view. */
export function keepTableInView(table: HTMLElement | null) {
  if (!table) return;
  const scroller = scrollParent(table);
  if (table.getBoundingClientRect().top < (scroller?.getBoundingClientRect().top ?? 0)) requestAnimationFrame(() => table.scrollIntoView({ block: "start" }));
}

/** The element that scrolls `el` (the app's content area). */
export function scrollParent(el: HTMLElement): HTMLElement | null {
  for (let p = el.parentElement; p; p = p.parentElement) {
    if (/(auto|scroll)/.test(getComputedStyle(p).overflowY)) return p;
  }
  return null;
}

/**
 * Beside a table of steps grouped in segments (rows with `data-seg`), the segment shown on the
 * map: the one under the pointer, else (pointer outside the table) the one whose first opened row
 * (`rows`) is at the top of the view. `onFocus` follows them only when `active`.
 */
export function useFollowPointer(
  table: RefObject<HTMLTableElement | null>,
  active: boolean,
  rows: string,
  onFocus: (segment: number) => void,
  deps: unknown[],
) {
  useEffect(() => {
    const el = table.current;
    if (!active || !el) return;
    const scroller = scrollParent(el);
    let frameId = 0;
    // Last pointer position over the table, null once it left.
    let pointer: { x: number; y: number } | null = null;
    const segAt = (target: Element | null) => {
      const seg = target?.closest<HTMLElement>("tr[data-seg]")?.dataset.seg;
      return seg != null && el.contains(target) ? Number(seg) : null;
    };
    const update = () => {
      frameId = 0;
      if (pointer) {
        // Scrolling under a still pointer: the row now beneath it.
        const seg = segAt(document.elementFromPoint(pointer.x, pointer.y));
        if (seg != null) return onFocus(seg);
      }
      const lines = el.querySelectorAll<HTMLTableRowElement>(rows);
      if (!lines.length) return;
      const top = (scroller?.getBoundingClientRect().top ?? 0) + 40;
      // Rows are in order: the first one ending below the top of the view.
      let lo = 0;
      let hi = lines.length - 1;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (lines[mid].getBoundingClientRect().bottom > top) hi = mid;
        else lo = mid + 1;
      }
      onFocus(Number(lines[lo].dataset.seg));
    };
    const onScroll = () => {
      if (!frameId) frameId = requestAnimationFrame(update);
    };
    const onOver = (e: PointerEvent) => {
      pointer = { x: e.clientX, y: e.clientY };
      const seg = segAt(e.target as Element);
      if (seg != null) onFocus(seg);
    };
    const onMove = (e: PointerEvent) => {
      pointer = { x: e.clientX, y: e.clientY };
    };
    const onLeave = () => {
      pointer = null;
    };
    update();
    scroller?.addEventListener("scroll", onScroll, { passive: true });
    el.addEventListener("pointerover", onOver);
    el.addEventListener("pointermove", onMove, { passive: true });
    el.addEventListener("pointerleave", onLeave);
    return () => {
      scroller?.removeEventListener("scroll", onScroll);
      el.removeEventListener("pointerover", onOver);
      el.removeEventListener("pointermove", onMove);
      el.removeEventListener("pointerleave", onLeave);
      cancelAnimationFrame(frameId);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, rows, ...deps]);
}
