import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { labelSlug, slotLabelKey, withoutColon } from "../src/lib/pob-label-slug";

const ROOTS = ["src-tauri/resources/pob", "src-tauri/resources/pob1"];
const SOCKET_LABELS = ["Abyssal #1", "Jewel #1", "Socket", "Socket #1"];
const CATALOGUE = "messages/pob/en.json";
const check = process.argv.includes("--check");

const labels = new Map<string, string>();
function add(prefix: string, text: string) {
  const slug = labelSlug(text);
  if (slug && !/[{}\\]/.test(text) && !labels.has(prefix + slug)) labels.set(prefix + slug, text);
}

for (const root of ROOTS) {
  const read = (path: string) => readFileSync(`${root}/${path}`, "utf8");

  for (const [, label] of read("Modules/BuildDisplayStats.lua").matchAll(/\blabel = "([^"]+)"/g)) add("pob_stat_", label);

  const baseSlots = /local baseSlots = \{([^}]+)\}/.exec(read("Classes/ItemsTab.lua"));
  if (!baseSlots) throw new Error(`no baseSlots list in ${root}/Classes/ItemsTab.lua`);
  const slots = [...baseSlots[1].matchAll(/"([^"]+)"/g)].map(([, name]) => name);
  for (const label of [...slots, ...SOCKET_LABELS]) {
    const key = slotLabelKey(label).key;
    if (!labels.has(key)) labels.set(key, label.replace(/\d+/, "{n}"));
  }

  for (const [, text] of read("Modules/ConfigOptions.lua").matchAll(/\b(?:label|section) = "([^"]+)"/g)) add("pob_config_", withoutColon(text.split(/ {8,}/)[0].trim()));

  for (const [, label] of read("Modules/CalcSections.lua").matchAll(/\blabel = "([^"]+)"/g)) add("pob_calc_", withoutColon(label));
}

const sorted = [...labels].sort(([a], [b]) => (a < b ? -1 : 1));
const body = sorted.map(([key, text]) => `  ${JSON.stringify(key)}: ${JSON.stringify(text)}`).join(",\n");
const next = `{\n  "$schema": "https://inlang.com/schema/inlang-message-format",\n${body}\n}\n`;
const current = existsSync(CATALOGUE) ? readFileSync(CATALOGUE, "utf8").replaceAll("\r\n", "\n") : "";

if (current === next) {
  console.log(`${labels.size} PoB labels, ${CATALOGUE} is current`);
} else if (check) {
  console.error(`${CATALOGUE} does not match the vendored PoB labels. Run: bun run i18n:labels`);
  process.exit(1);
} else {
  writeFileSync(CATALOGUE, next);
  console.log(`wrote ${labels.size} PoB labels to ${CATALOGUE}`);
}
