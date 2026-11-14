export type PanelId = "gear" | "skills" | "health" | "defences" | "passives";
export type Column = "left" | "right";
export type Layout = Record<Column, PanelId[]>;

const KEY = "pob-redux:overview-layout";
const COLUMNS: Column[] = ["left", "right"];
const DEFAULT: Layout = { left: ["gear", "skills"], right: ["health", "defences", "passives"] };

function load(): Layout {
  let saved: Partial<Layout> = {};
  try {
    saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {}
  const known = new Set<PanelId>([...DEFAULT.left, ...DEFAULT.right]);
  const seen = new Set<PanelId>();
  const out: Layout = { left: [], right: [] };
  for (const c of COLUMNS) {
    for (const id of Array.isArray(saved[c]) ? saved[c] : []) {
      if (known.has(id) && !seen.has(id)) {
        seen.add(id);
        out[c].push(id);
      }
    }
  }
  for (const c of COLUMNS) for (const id of DEFAULT[c]) if (!seen.has(id)) out[c].push(id);
  return out;
}

class OverviewLayout {
  columns = $state<Layout>(load());

  get changed(): boolean {
    return COLUMNS.some((c) => this.columns[c].join() !== DEFAULT[c].join());
  }

  where(id: PanelId): { column: Column; index: number } {
    const column = COLUMNS.find((c) => this.columns[c].includes(id)) ?? "left";
    return { column, index: this.columns[column].indexOf(id) };
  }

  /** Puts `id` at `index` among the other panels of `column`; false when nothing moved. */
  move(id: PanelId, column: Column, index: number): boolean {
    const from = this.where(id);
    const rest = this.columns[column].filter((p) => p !== id);
    const at = Math.max(0, Math.min(index, rest.length));
    if (from.column === column && from.index === at) return false;
    const next: Layout = { left: this.columns.left.filter((p) => p !== id), right: this.columns.right.filter((p) => p !== id) };
    next[column] = [...rest.slice(0, at), id, ...rest.slice(at)];
    this.columns = next;
    return true;
  }

  nudge(id: PanelId, key: string): boolean {
    const { column, index } = this.where(id);
    if (key === "ArrowUp") return this.move(id, column, index - 1);
    if (key === "ArrowDown") return this.move(id, column, index + 1);
    const other: Column = column === "left" ? "right" : "left";
    if ((key === "ArrowLeft" && column === "right") || (key === "ArrowRight" && column === "left")) return this.move(id, other, index);
    return false;
  }

  save() {
    try {
      localStorage.setItem(KEY, JSON.stringify(this.columns));
    } catch {}
  }

  reset() {
    this.columns = { left: [...DEFAULT.left], right: [...DEFAULT.right] };
    this.save();
  }
}

export const overviewLayout = new OverviewLayout();
