import { readFileSync, writeFileSync } from "node:fs";
import { labelSlug, slotLabelKey } from "../src/lib/pob-label-slug";

const ROOTS = ["src-tauri/resources/pob", "src-tauri/resources/pob1"];
const SOCKET_LABELS = ["Abyssal #1", "Jewel #1", "Socket", "Socket #1"];
const MESSAGES = "messages/en.json";
const check = process.argv.includes("--check");

const labels = new Map<string, string>();
function add(key: string, text: string) {
  const seen = labels.get(key);
  if (seen && seen !== text) throw new Error(`${key} is the key for both "${seen}" and "${text}"`);
  labels.set(key, text);
}

for (const root of ROOTS) {
  const stats = readFileSync(`${root}/Modules/BuildDisplayStats.lua`, "utf8");
  for (const [, label] of stats.matchAll(/\blabel = "([^"]+)"/g)) add(`pob_stat_${labelSlug(label)}`, label);

  const baseSlots = /local baseSlots = \{([^}]+)\}/.exec(readFileSync(`${root}/Classes/ItemsTab.lua`, "utf8"));
  if (!baseSlots) throw new Error(`no baseSlots list in ${root}/Classes/ItemsTab.lua`);
  const slots = [...baseSlots[1].matchAll(/"([^"]+)"/g)].map(([, name]) => name);
  for (const label of [...slots, ...SOCKET_LABELS]) add(slotLabelKey(label).key, label.replace(/\d+/, "{n}"));
}

const raw = readFileSync(MESSAGES, "utf8");
const existing = JSON.parse(raw) as Record<string, string>;
const missing = [...labels].filter(([key]) => !(key in existing)).sort(([a], [b]) => a.localeCompare(b));

if (!missing.length) {
  console.log(`${labels.size} PoB labels, all in ${MESSAGES}`);
} else if (check) {
  console.error(`${MESSAGES} has no key for ${missing.length} PoB labels. Run: bun run i18n:labels`);
  for (const [key, label] of missing) console.error(`  ${key}: ${label}`);
  process.exit(1);
} else {
  const eol = raw.includes("\r\n") ? "\r\n" : "\n";
  const end = raw.lastIndexOf("}");
  const head = raw.slice(0, end).trimEnd();
  const lines = missing.map(([key, label]) => `  ${JSON.stringify(key)}: ${JSON.stringify(label)}`);
  writeFileSync(MESSAGES, `${head},${eol}${eol}${lines.join(`,${eol}`)}${eol}}${eol}`);
  console.log(`added ${missing.length} PoB labels to ${MESSAGES}`);
}
