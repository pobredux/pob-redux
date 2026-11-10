import { m } from "$lib/paraglide/messages";
import { labelSlug, slotLabelKey } from "$lib/pob-label-slug";

const messages = m as unknown as Record<string, ((params: { n: string }) => string) | undefined>;

export function statLabel(english: string): string {
  return messages[`pob_stat_${labelSlug(english)}`]?.({ n: "" }) ?? english;
}

export function slotLabel(english: string): string {
  const { key, n } = slotLabelKey(english);
  return messages[key]?.({ n: n ?? "" }) ?? english;
}

const LEADING_CODES = /^((?:\^x[0-9a-fA-F]{6}|\^\d)*)([\s\S]*)$/;

export function statLabelText(pobText: string | null | undefined): string {
  const [, codes, text] = LEADING_CODES.exec(pobText ?? "")!;
  const colon = /:\s*$/.test(text);
  const label = statLabel(text.replace(/:\s*$/, "").trim());
  return `${codes}${label}${colon ? ":" : ""}`;
}
