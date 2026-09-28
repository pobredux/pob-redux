import type { BreakdownSection } from "$lib/engine.svelte";
import { stripPobText } from "$lib/pobtext";

export type CalcBreakdownRef = {
  section: number;
  sub: number;
  row: number;
  col: number;
  actor: "player" | "minion";
};

export type BreakdownPosition = { x: number; y: number };
export type BreakdownViewport = { width: number; height: number };
export type BreakdownAnchor = { left: number; right: number; top: number; bottom: number };
export type BreakdownBlocker = BreakdownPosition & { key: string; width: number; height: number; ready: boolean };

export function calcBreakdownKey(ref: CalcBreakdownRef) {
  return `${ref.actor}:${ref.section}:${ref.sub}:${ref.row}:${ref.col}`;
}

export function calcBreakdownWidth(sections: BreakdownSection[], title: string, viewportWidth: number) {
  const viewportMax = Math.max(320, Math.min(780, viewportWidth - 24));
  let ideal = stripPobText(title).length * 7 + 100;
  for (const section of sections) {
    if (section.type === "text") {
      for (const line of section.lines) ideal = Math.max(ideal, stripPobText(line).length * 6.5 + 48);
    } else if (section.type === "table") {
      const columns = section.cols.filter((column) => section.rows.some((row) => stripPobText(row[column.key] ?? "").trim()));
      const tableWidth = columns.reduce((width, column) => {
        const longest = Math.max(column.label.length, ...section.rows.map((row) => stripPobText(row[column.key] ?? "").length));
        return width + Math.min(longest, 42) * 6.2 + 20;
      }, 2);
      ideal = Math.max(ideal, tableWidth + 24);
    } else {
      ideal = Math.max(ideal, 504);
    }
  }
  return Math.min(viewportMax, Math.max(Math.min(560, viewportMax), Math.ceil(ideal)));
}

export function clampBreakdownPosition(position: BreakdownPosition, width: number, height: number, viewport: BreakdownViewport) {
  return {
    x: Math.max(12, Math.min(position.x, Math.max(12, viewport.width - width - 12))),
    y: Math.max(42, Math.min(position.y, Math.max(42, viewport.height - height - 12))),
  };
}

export function chooseBreakdownPosition(
  anchor: BreakdownAnchor,
  width: number,
  height: number,
  viewport: BreakdownViewport,
  blockers: BreakdownBlocker[],
  excludeKey?: string,
) {
  const maxX = Math.max(12, viewport.width - width - 12);
  const maxY = Math.max(42, viewport.height - height - 12);
  const clampX = (value: number) => Math.max(12, Math.min(value, maxX));
  const clampY = (value: number) => Math.max(42, Math.min(value, maxY));
  const rightX = clampX(anchor.right + 8);
  const leftX = clampX(anchor.left - width - 8);
  const baseY = clampY(anchor.top - 8);
  const visible = blockers.filter((popup) => popup.key !== excludeKey && popup.ready && popup.height > 0);
  if (visible.length === 0 || height === 0) {
    const x = anchor.right + 8 + width <= viewport.width - 12 ? rightX : leftX;
    return { x, y: baseY };
  }

  const xs = [...new Set([rightX, leftX])];
  const ys = new Set([baseY]);
  for (const popup of visible) {
    ys.add(clampY(popup.y - height - 8));
    ys.add(clampY(popup.y + popup.height + 8));
  }

  const rowY = (anchor.top + anchor.bottom) / 2;
  const candidates = xs.flatMap((x) => [...ys].map((y) => {
    let overlap = 0;
    for (const popup of visible) {
      const overlapX = Math.max(0, Math.min(x + width, popup.x + popup.width + 8) - Math.max(x, popup.x - 8));
      const overlapY = Math.max(0, Math.min(y + height, popup.y + popup.height + 8) - Math.max(y, popup.y - 8));
      overlap += overlapX * overlapY;
    }
    const rowGap = rowY < y ? y - rowY : rowY > y + height ? rowY - y - height : 0;
    const distance = Math.abs(y - baseY) + rowGap * 3 + (x === rightX ? 0 : 4);
    return { x, y, overlap, distance };
  }));
  const best = candidates.sort((a, b) => Number(a.overlap > 0) - Number(b.overlap > 0) || a.overlap - b.overlap || a.distance - b.distance)[0];
  return { x: best.x, y: best.y };
}
