import type { PartyBuff, PartyMod } from "$lib/engine.svelte";

type Tag = PartyMod["tags"][number];

const SMALL_WORDS = new Set(["On", "As", "Of", "To", "Per", "By", "With", "From", "And", "Or", "For", "In", "At"]);
const PERCENT_NAME = /(Resist|ResistMax|Chance|Multiplier|Penetration|Percent|Reduction|Effect)$|GainAs/;
const FLAG_LABELS: Record<string, string> = { Dot: "Damage over Time" };

export function words(name: string): string {
  return name
    .replace(/_/g, " ")
    .replace(/([a-z0-9'])([A-Z])/g, "$1 $2")
    .replace(/([A-Z]+)([A-Z][a-z])/g, "$1 $2")
    .split(" ")
    .map((w, i) => (i > 0 && SMALL_WORDS.has(w) ? w.toLowerCase() : w))
    .join(" ")
    .replace(/\bResist\b/g, "Resistance")
    .trim();
}

function fmt(v: number): string {
  return String(Number(Math.abs(v).toFixed(2)));
}

function tagText(t: Tag): string | null {
  const s = (k: string) => (t[k] === undefined ? undefined : String(t[k]));
  const neg = t.neg === true || t.neg === "true";
  switch (t.type) {
    case "Condition": {
      const v = s("var") ?? s("varList");
      return v ? `while ${neg ? "not " : ""}${words(v)}` : null;
    }
    case "ActorCondition": {
      const v = s("var");
      return v ? `while ${s("actor") ?? "the target"} is ${neg ? "not " : ""}${words(v)}` : null;
    }
    case "Multiplier": {
      const v = s("var");
      if (!v) return null;
      const div = Number(t.div ?? 1);
      const limit = s("limit") ?? (s("limitVar") && words(s("limitVar")!));
      return `per ${div > 1 ? `${div} ` : ""}${words(v)}${limit ? ` (up to ${limit})` : ""}`;
    }
    case "MultiplierThreshold": {
      const v = s("var");
      if (!v) return null;
      if (s("threshold")) return `with at least ${s("threshold")} ${words(v)}`;
      return s("thresholdVar") ? `while ${words(v)} is at least ${words(s("thresholdVar")!)}` : null;
    }
    case "PerStat": {
      const stat = s("stat");
      return stat ? `per ${s("div") ?? 1} ${words(stat)}` : null;
    }
    case "PercentStat": {
      const stat = s("stat");
      if (!stat) return null;
      const whose = t.actor === "partyMembers" ? "party members' " : t.actor === "enemy" ? "the enemy's " : "";
      return `per ${s("percent") ?? 1}% of ${whose}${words(stat)}`;
    }
    case "StatThreshold": {
      const stat = s("stat");
      return stat ? `while ${words(stat)} is at least ${s("threshold") ?? 0}` : null;
    }
    case "Limit":
      return s("limit") ? `(limit ${s("limit")})` : null;
    default:
      return null;
  }
}

function suffix(mod: PartyMod): string {
  const parts = mod.tags.map(tagText).filter((x): x is string => !!x);
  const flags = [...mod.flags, ...mod.keywordFlags].map((f) => FLAG_LABELS[f] ?? words(f));
  if (flags.length) parts.push(`(${flags.join(", ")})`);
  return parts.length ? ` ${parts.join(" ")}` : "";
}

/** Scale a value the way PoB's ScaleAddMod does: whole numbers truncate (curses round), fractions keep one decimal. */
function scaled(mod: PartyMod, scale: number, round: boolean): PartyMod["value"] {
  if (typeof mod.value !== "number" || scale === 1 || mod.tags.some((t) => t.unscalable)) return mod.value;
  const v = mod.value * scale;
  if (!Number.isInteger(mod.value)) return Math.floor(v * 10) / 10;
  if (round) return Math.sign(v) * Math.round(Math.abs(v));
  return Math.trunc(Math.round(v * 100) / 100);
}

function line(mod: PartyMod): string | null {
  if (mod.name === "Dummy") return null;
  let name = mod.name;
  const against = /^Self[A-Z]/.test(name);
  if (against) name = name.slice(4);
  const tail = `${against ? " against the enemy" : ""}${suffix(mod)}`;
  const v = mod.value;

  if (name.startsWith("Multiplier:")) return `${words(name.slice(11))}: ${typeof v === "number" ? fmt(v) : v}${tail}`;
  if (mod.type === "FLAG") return `${words(name)}${tail}`;
  if (typeof v !== "number") return `${words(name)}${v === null ? "" : `: ${v}`}${tail}`;

  const percent = PERCENT_NAME.test(name);
  const max = /^(\w+)ResistMax$/.exec(name);
  const subject = max ? `maximum ${words(max[1])} Resistance` : words(name.replace(/Percent$/, ""));
  switch (mod.type) {
    case "INC":
      return `${fmt(v)}% ${v < 0 ? "reduced" : "increased"} ${subject}${tail}`;
    case "MORE":
      return `${fmt(v)}% ${v < 0 ? "less" : "more"} ${subject}${tail}`;
    case "BASE":
      return `${v < 0 ? "-" : "+"}${fmt(v)}${percent ? "%" : ""} ${subject}${tail}`;
    case "OVERRIDE":
      return `${subject} is ${fmt(v)}${tail}`;
    default:
      return `${subject}: ${fmt(v)}${tail}`;
  }
}

export function modLines(mods: PartyMod[], effect: number | null = null, round = false): string[] {
  const scale = effect && effect > 0 ? effect / 100 : 1;
  const list = mods.map((mod) => ({ ...mod, value: scaled(mod, scale, round) }));
  const used = new Set<number>();
  const out: string[] = [];
  list.forEach((mod, i) => {
    if (used.has(i)) return;
    const min = /^(\w+)Min$/.exec(mod.name);
    if (min && mod.type === "BASE" && typeof mod.value === "number") {
      const key = JSON.stringify([mod.tags, mod.flags, mod.keywordFlags]);
      const j = list.findIndex(
        (o, k) => !used.has(k) && k !== i && o.name === `${min[1]}Max` && o.type === "BASE" && JSON.stringify([o.tags, o.flags, o.keywordFlags]) === key,
      );
      if (j >= 0 && typeof list[j].value === "number") {
        used.add(j);
        out.push(`Adds ${fmt(mod.value)} to ${fmt(list[j].value as number)} ${words(min[1])} Damage${suffix(mod)}`);
        return;
      }
    }
    const text = line(mod);
    if (text) out.push(text);
  });
  return [...new Set(out)];
}

export function buffLines(buff: PartyBuff): string[] {
  return modLines(buff.mods, buff.effect, buff.kind === "curse");
}
