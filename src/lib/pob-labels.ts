import { m } from "$lib/paraglide/messages";
import * as english from "$lib/paraglide/messages/en.js";
import { labelSlug, slotLabelKey, withoutColon } from "$lib/pob-label-slug";

type Message = (params: { n: string }) => string;
const messages = m as unknown as Record<string, Message | undefined>;
const base = english as unknown as Record<string, Message | undefined>;

// An untranslated key returns the catalogue's English, which can differ from this game's PoB text in colour codes.
function translated(key: string, n = ""): string | null {
  const text = messages[key]?.({ n });
  return text && text !== base[key]?.({ n }) ? text : null;
}

function lookup(prefix: string, pobText: string): string {
  const text = translated(prefix + labelSlug(withoutColon(pobText)));
  if (!text) return pobText;
  return /:\s*$/.test(pobText) ? `${text}:` : text;
}

export const statLabel = (english: string) => lookup("pob_stat_", english);
export const configLabel = (pobText: string) => lookup("pob_config_", pobText);
export const calcLabel = (pobText: string) => lookup("pob_calc_", pobText);

export function slotLabel(english: string): string {
  const { key, n } = slotLabelKey(english);
  return translated(key, n ?? "") ?? english;
}

const LEADING_CODES = /^((?:\^x[0-9a-fA-F]{6}|\^\d)*)([\s\S]*)$/;

export function statLabelText(pobText: string | null | undefined): string {
  const [, codes, text] = LEADING_CODES.exec(pobText ?? "")!;
  return codes + statLabel(text.trim());
}
