export function labelSlug(english: string): string {
  return english
    .toLowerCase()
    .replace(/%/g, " pct ")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

export function slotLabelKey(english: string): { key: string; n: string | null } {
  const n = /\d+/.exec(english)?.[0] ?? null;
  return { key: `pob_slot_${labelSlug(english.replace(/\d+/, ""))}${n ? "_n" : ""}`, n };
}
