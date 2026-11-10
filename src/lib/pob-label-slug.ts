const COLOUR_CODES = /\^x[0-9a-fA-F]{6}|\^\d/g;

export function labelSlug(english: string): string {
  return english
    .replace(COLOUR_CODES, "")
    .toLowerCase()
    .replace(/%/g, " pct ")
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
}

export const withoutColon = (text: string) => text.replace(/:\s*$/, "");

export function slotLabelKey(english: string): { key: string; n: string | null } {
  const n = /\d+/.exec(english)?.[0] ?? null;
  return { key: `pob_slot_${labelSlug(english.replace(/\d+/, ""))}${n ? "_n" : ""}`, n };
}
