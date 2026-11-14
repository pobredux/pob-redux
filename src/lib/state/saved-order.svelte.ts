/** A user's order for a list of cards, kept in localStorage; with no `defaults`, an empty order means the app's own. */
export class SavedOrder {
  keys = $state<string[]>([]);

  constructor(
    private readonly storageKey: string,
    private readonly defaults: string[] = [],
  ) {
    let saved: unknown = [];
    try {
      saved = JSON.parse(localStorage.getItem(storageKey) ?? "[]");
    } catch {}
    const keys = Array.isArray(saved) ? saved.filter((k): k is string => typeof k === "string" && (!defaults.length || defaults.includes(k))) : [];
    this.keys = [...new Set([...keys, ...defaults])];
  }

  get changed(): boolean {
    return this.keys.join() !== this.defaults.join();
  }

  /** Stable: cards the order does not name keep their incoming order, after the named ones. */
  sort<T>(cards: T[], key: (card: T) => string): T[] {
    const rank = (k: string) => {
      const i = this.keys.indexOf(k);
      return i < 0 ? this.keys.length : i;
    };
    return cards.slice().sort((a, b) => rank(key(a)) - rank(key(b)));
  }

  /** Puts `key` at `index` among the other `shown` keys; keys not shown keep their place. */
  move(key: string, shown: string[], index: number): boolean {
    const others = shown.filter((k) => k !== key);
    const at = Math.max(0, Math.min(index, others.length));
    const next = [...others.slice(0, at), key, ...others.slice(at)];
    if (next.join() === shown.join()) return false;
    const keys = [...this.keys, ...shown.filter((k) => !this.keys.includes(k))];
    const slots = new Set(shown);
    let i = 0;
    this.keys = keys.map((k) => (slots.has(k) ? next[i++] : k));
    return true;
  }

  save() {
    try {
      if (this.keys.length && this.changed) localStorage.setItem(this.storageKey, JSON.stringify(this.keys));
      else localStorage.removeItem(this.storageKey);
    } catch {}
  }

  reset() {
    this.keys = [...this.defaults];
    this.save();
  }
}

export const statsOrder = new SavedOrder("pob-redux:stats-order", [
  "minion", "offence", "skill", "attributes", "resources", "mitigation", "resistances", "misc", "fulldps", "warnings", "notcounted",
]);

export const calcsOrder: Record<string, SavedOrder> = {
  offence: new SavedOrder("pob-redux:calcs-order:offence"),
  defence: new SavedOrder("pob-redux:calcs-order:defence"),
  other: new SavedOrder("pob-redux:calcs-order:other"),
};
