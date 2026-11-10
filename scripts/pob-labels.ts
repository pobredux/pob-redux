import { readFileSync, writeFileSync } from "node:fs";
import { labelSlug } from "../src/lib/pob-label-slug";

const SOURCES = ["src-tauri/resources/pob/Modules/BuildDisplayStats.lua", "src-tauri/resources/pob1/Modules/BuildDisplayStats.lua"];
const MESSAGES = "messages/en.json";
const check = process.argv.includes("--check");

const labels = new Map<string, string>();
for (const path of SOURCES) {
  for (const [, label] of readFileSync(path, "utf8").matchAll(/\blabel = "([^"]+)"/g)) {
    const key = `pob_stat_${labelSlug(label)}`;
    const seen = labels.get(key);
    if (seen && seen !== label) throw new Error(`${key} is the key for both "${seen}" and "${label}"`);
    labels.set(key, label);
  }
}

const raw = readFileSync(MESSAGES, "utf8");
const existing = JSON.parse(raw) as Record<string, string>;
const missing = [...labels].filter(([key]) => !(key in existing)).sort(([a], [b]) => a.localeCompare(b));

if (!missing.length) {
  console.log(`${labels.size} PoB stat labels, all in ${MESSAGES}`);
} else if (check) {
  console.error(`${MESSAGES} has no key for ${missing.length} PoB stat labels. Run: bun run i18n:labels`);
  for (const [key, label] of missing) console.error(`  ${key}: ${label}`);
  process.exit(1);
} else {
  const eol = raw.includes("\r\n") ? "\r\n" : "\n";
  const end = raw.lastIndexOf("}");
  const head = raw.slice(0, end).trimEnd();
  const lines = missing.map(([key, label]) => `  ${JSON.stringify(key)}: ${JSON.stringify(label)}`);
  writeFileSync(MESSAGES, `${head},${eol}${eol}${lines.join(`,${eol}`)}${eol}}${eol}`);
  console.log(`added ${missing.length} PoB stat labels to ${MESSAGES}`);
}
