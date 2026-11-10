import { m } from "$lib/paraglide/messages";
import { labelSlug } from "$lib/pob-label-slug";

const messages = m as unknown as Record<string, (() => string) | undefined>;

export function statLabel(english: string): string {
  return messages[`pob_stat_${labelSlug(english)}`]?.() ?? english;
}

const LEADING_CODES = /^((?:\^x[0-9a-fA-F]{6}|\^\d)*)([\s\S]*)$/;

export function statLabelText(pobText: string | null | undefined): string {
  const [, codes, text] = LEADING_CODES.exec(pobText ?? "")!;
  const colon = /:\s*$/.test(text);
  const label = statLabel(text.replace(/:\s*$/, "").trim());
  return `${codes}${label}${colon ? ":" : ""}`;
}
