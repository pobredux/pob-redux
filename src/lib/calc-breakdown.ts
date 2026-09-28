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
  const centerX = clampX((anchor.left + anchor.right - width) / 2);
  const baseY = clampY(anchor.top - 8);
  const visible = blockers.filter((popup) => popup.key !== excludeKey && popup.ready && popup.height > 0);
  if (height === 0) {
    const rightFits = anchor.right + 8 + width <= viewport.width - 12;
    const leftFits = anchor.left - width - 8 >= 12;
    const x = rightFits ? rightX : leftFits ? leftX : centerX;
    return { x, y: baseY };
  }

  const xs = [...new Set([rightX, leftX, centerX])];
  const ys = new Set([baseY, clampY(anchor.top - height - 8), clampY(anchor.bottom + 8)]);
  for (const popup of visible) {
    ys.add(clampY(popup.y - height - 8));
    ys.add(clampY(popup.y + popup.height + 8));
  }

  const overlapArea = (x: number, y: number, blocker: BreakdownAnchor) => {
    const overlapX = Math.max(0, Math.min(x + width, blocker.right + 8) - Math.max(x, blocker.left - 8));
    const overlapY = Math.max(0, Math.min(y + height, blocker.bottom + 8) - Math.max(y, blocker.top - 8));
    return overlapX * overlapY;
  };
  const anchorCenterX = (anchor.left + anchor.right) / 2;
  const candidates = xs.flatMap((x) => [...ys].map((y) => {
    const anchorOverlap = overlapArea(x, y, anchor);
    let popupOverlap = 0;
    for (const popup of visible) {
      popupOverlap += overlapArea(x, y, {
        left: popup.x,
        right: popup.x + popup.width,
        top: popup.y,
        bottom: popup.y + popup.height,
      });
    }
    const gapX = x >= anchor.right ? x - anchor.right : x + width <= anchor.left ? anchor.left - x - width : 0;
    const gapY = y >= anchor.bottom ? y - anchor.bottom : y + height <= anchor.top ? anchor.top - y - height : 0;
    const alignedSide = gapX > 0 && gapY === 0;
    const alignment = alignedSide ? Math.abs(y - baseY) : Math.abs(x + width / 2 - anchorCenterX);
    const distance = gapX + gapY + alignment * 0.1 + (alignedSide ? (x === leftX ? 4 : 0) : 8);
    return { x, y, anchorOverlap, popupOverlap, distance };
  }));
  const best = candidates.sort((a, b) =>
    Number(a.anchorOverlap > 0) - Number(b.anchorOverlap > 0)
    || Number(a.popupOverlap > 0) - Number(b.popupOverlap > 0)
    || a.anchorOverlap - b.anchorOverlap
    || a.popupOverlap - b.popupOverlap
    || a.distance - b.distance
  )[0];
  return { x: best.x, y: best.y };
}
