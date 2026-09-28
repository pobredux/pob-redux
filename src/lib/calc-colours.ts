import { parsePobText, type Span } from "$lib/pobtext";

export const ROW_COLOURS: Record<string, Record<string, string>> = {
  Resist: {
    "Fire Resist": "^xB97123",
    "Cold Resist": "^x3F6DB3",
    "Lightning Resist": "^xADAA47",
    "Chaos Resist": "^xD02090",
  },
  Attributes: {
    Strength: "^xE05030",
    Dexterity: "^x70FF70",
    Intelligence: "^x7070FF",
    Omniscience: "^xFFFF77",
  },
};

export const CHARGE_COLOURS: Record<string, string> = {
  Endurance: "^xFF9922",
  Frenzy: "^x33FF77",
  Power: "^x7070FF",
};

const DAMAGE_DETAILS = [
  "Damage over Time(?: Multiplier)?",
  "Damage Reduction",
  "Damage Taken",
  "Damage",
  "DoT(?: Multiplier)?",
  "Pen(?:etration)?",
  "Resist(?:ance)?",
  "Reduction",
  "Mitigation",
  "Exposure",
  "Hit(?: Damage)?",
  "DPS",
].join("|");

const typedDamage = (type: string) => `${type}(?:\\s+(?:${DAMAGE_DETAILS}))?`;
const colourRule = (colour: string, labels: string) => ({ colour, pattern: new RegExp(`\\b(?:${labels})\\b`, "g") });
const LABEL_RULES = [
  colourRule("var(--c-physical)", `${typedDamage("Physical")}|Bleed(?:ing|s)?|Corrupted Blood`),
  colourRule("var(--c-fire)", `${typedDamage("Fire")}|Ignite[ds]?|Igniting|Burning(?: Damage)?|Scorch(?:ed)?`),
  colourRule("var(--c-cold)", `${typedDamage("Cold")}|Chill(?:ed|ing)?|Freeze|Freez(?:es|ing)|Frozen|Brittle`),
  colourRule("var(--c-lightning)", `${typedDamage("Lightning")}|Shock(?:ed|ing|s)?|Sap(?:ped)?`),
  colourRule("var(--c-chaos)", `${typedDamage("Chaos")}|Poison(?:ed|ing|s)?|Decay`),
  colourRule("var(--c-life)", "Strength"),
  colourRule("var(--ok)", "Dexterity|Frenzy(?: Charges?)?"),
  colourRule("var(--c-mana)", "Intelligence|Power(?: Charges?)?"),
  colourRule("var(--c-rare)", "Omniscience"),
  colourRule("var(--warn)", "Endurance(?: Charges?)?"),
];

export function parseCalcText(value: string | null | undefined): Span[] {
  return parsePobText(value).flatMap((span) => {
    if (span.color !== null) return [span];
    const matches = LABEL_RULES.flatMap((rule) => [...span.text.matchAll(rule.pattern)].map((match) => ({
      colour: rule.colour,
      index: match.index ?? 0,
      text: match[0],
    }))).sort((a, b) => a.index - b.index || b.text.length - a.text.length);
    const parts: Span[] = [];
    let offset = 0;
    for (const match of matches) {
      if (match.index < offset) continue;
      if (match.index > offset) parts.push({ text: span.text.slice(offset, match.index), color: span.color });
      parts.push({ text: match.text, color: match.colour });
      offset = match.index + match.text.length;
    }
    if (offset < span.text.length) parts.push({ text: span.text.slice(offset), color: span.color });
    return parts;
  });
}
