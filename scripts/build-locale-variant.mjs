// Generates the data files for a locale variant from an existing locale.
//
//   node scripts/build-locale-variant.mjs --from zh-CN --to zh-TW [options]
//
//   --opencc <config>   OpenCC conversion config, default "s2twp"
//   --python <path>     Python interpreter with the `opencc` package; falls
//                       back to $OPENCC_PYTHON, then "python"
//   --game <tag>        game dictionary used for gap filling, default "poe1"
//   --fill-gaps         copy dictionary entries present in the source locale
//                       but missing from the target locale into the override
//                       table (converted), so a smaller upstream wordlist does
//                       not leave newer PoB labels untranslated
//   --extra <file>      manual corrections already written in the target
//                       language; merged last and never converted
//   --overrides-only    skip messages / sidebar tables
//
// This is a build-time tool, not a runtime dependency. Its output is ordinary
// data: src/lib/i18n/dictionaries.ts and Paraglide discover the generated
// files by path, so shipping a language never requires a framework change.
// Conversion is delegated to OpenCC so the target keeps its own vocabulary
// (e.g. Taiwan phrase substitutions), not just traditional glyphs.

import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";

const args = process.argv.slice(2);
const argOf = (flag, fallback) => {
  const i = args.indexOf(flag);
  return i >= 0 && args[i + 1] ? args[i + 1] : fallback;
};
const has = (flag) => args.includes(flag);

const repo = path.resolve(import.meta.dirname, "..");
const from = argOf("--from", "zh-CN");
const to = argOf("--to", "zh-TW");
const game = argOf("--game", "poe1");
const opencc = argOf("--opencc", "s2twp");
const python = argOf("--python", process.env.OPENCC_PYTHON ?? "python");
const extraFile = argOf("--extra", "");

const readJson = (file) => JSON.parse(fs.readFileSync(file, "utf8"));
// Optional hand-written corrections, already in the target language:
//   { "messages": { "<key>": "..." }, "overrides": { "_ui": {...}, ... } }
// A flat override-shaped object is also accepted.
const extra = extraFile ? readJson(path.resolve(extraFile)) : {};
const writeJson = (file, value) => {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, JSON.stringify(value, null, 2) + "\n", "utf8");
};

const CONVERTER = `
import json, sys
from opencc import OpenCC
cc = OpenCC(sys.argv[1])
data = json.load(sys.stdin)
sys.stdout.write(json.dumps([cc.convert(x) if isinstance(x, str) else x for x in data], ensure_ascii=False))
`;

/** Find an interpreter that can import opencc (the default one may not). */
let converterPython;
function findConverter() {
  const candidates = [
    python,
    "python",
    "python3",
    ...(process.env.LOCALAPPDATA
      ? [path.join(process.env.LOCALAPPDATA, "Programs/Python/Python312/python.exe")]
      : []),
  ];
  for (const candidate of [...new Set(candidates)]) {
    const probe = spawnSync(candidate, ["-c", "import opencc"], { encoding: "utf8" });
    if (!probe.error && probe.status === 0) return candidate;
  }
  return python;
}

/** Convert a batch of strings in one interpreter launch. */
function convert(values) {
  if (!values.length) return [];
  converterPython ??= findConverter();
  const result = spawnSync(converterPython, ["-c", CONVERTER, opencc], {
    input: JSON.stringify(values),
    encoding: "utf8",
    maxBuffer: 128 * 1024 * 1024,
    env: { ...process.env, PYTHONIOENCODING: "utf-8" },
  });
  if (result.error || result.status !== 0) {
    console.error(`OpenCC conversion failed (interpreter: ${converterPython}).`);
    console.error(result.error?.message ?? result.stderr?.trim() ?? `exit ${result.status}`);
    console.error("Install opencc-python-reimplemented, or pass --python <path>.");
    process.exit(1);
  }
  const out = JSON.parse(result.stdout);
  if (out.length !== values.length) throw new Error("converter returned a different number of values");
  return out;
}

/** Convert every string value of a flat object, preserving keys and order. */
function convertMap(map) {
  const keys = Object.keys(map);
  const converted = convert(keys.map((k) => map[k]));
  return Object.fromEntries(keys.map((k, i) => [k, converted[i]]));
}

const localeDir = (locale) => path.join(repo, "src/lib/i18n/data/locales", locale);
const overridesDir = path.join(repo, "src/lib/i18n/data/overrides");

if (!has("--overrides-only")) {
  const messagesFrom = path.join(repo, "messages", `${from}.json`);
  const labelsFrom = path.join(repo, "src/lib/i18n/stat-labels/tables", `${from}.json`);
  const messages = { ...convertMap(readJson(messagesFrom)), ...(extra.messages ?? {}) };
  writeJson(path.join(repo, "messages", `${to}.json`), messages);
  console.log(`wrote messages/${to}.json (${Object.keys(messages).length} keys)`);
  writeJson(path.join(repo, "src/lib/i18n/stat-labels/tables", `${to}.json`), convertMap(readJson(labelsFrom)));
  console.log(`wrote stat-labels/tables/${to}.json (${Object.keys(readJson(labelsFrom)).length} keys)`);
}

// Build the override table: converted manual corrections, then dictionary
// gap fills, then target-language-only corrections from --extra.
const sourceOverrides = readJson(path.join(overridesDir, `${from}.json`));
const result = {
  _comment:
    "Corrections applied last by scripts/build-game-dictionary.mjs. Top-level keys are game-data entries; _ui holds interface labels shared by all games; _<game> groups are scoped to one game. One file per locale: data/overrides/<locale>.json.",
};
// Convert each grouped section (_ui, _poe2, ...) as a unit. Nested keys that
// start with "_" are human comments and are dropped rather than translated.
for (const [key, value] of Object.entries(sourceOverrides)) {
  if (key === "_comment") continue;
  if (key.startsWith("_")) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      const entries = Object.entries(value).filter(([k]) => !k.startsWith("_"));
      const converted = convert(entries.map(([, v]) => String(v)));
      result[key] = Object.fromEntries(entries.map(([k], i) => [k, converted[i]]));
    }
    continue;
  }
  result[key] = value;
}
const gameValues = Object.entries(result).filter(([k]) => !k.startsWith("_"));
if (gameValues.length) {
  const converted = convert(gameValues.map(([, v]) => v));
  gameValues.forEach(([k], i) => (result[k] = converted[i]));
}

let filled = { ui: 0, game: 0 };
if (has("--fill-gaps")) {
  const pairs = [
    ["_ui", "ui.json", result._ui],
    [null, `${game}.json`, result],
  ];
  for (const [scope, file, target] of pairs) {
    const sourceDict = readJson(path.join(localeDir(from), file));
    const targetDict = readJson(path.join(localeDir(to), file));
    for (const mapName of ["exact", "patterns"]) {
      const gaps = Object.entries(sourceDict[mapName]).filter(([key]) => !(key in targetDict[mapName]));
      if (!gaps.length) continue;
      const converted = convert(gaps.map(([, value]) => value));
      gaps.forEach(([key], i) => {
        if (!(key in target)) target[key] = converted[i];
      });
      filled[scope ? "ui" : "game"] += gaps.length;
    }
  }
}

const manual = extra.overrides ?? extra;
for (const [key, value] of Object.entries(manual)) {
  if (key === "messages" || key === "overrides") continue;
  if (key.startsWith("_")) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
      result[key] = { ...(result[key] ?? {}), ...value };
    }
    continue;
  }
  result[key] = value;
}

writeJson(path.join(overridesDir, `${to}.json`), result);
const sections = Object.entries(result)
  .filter(([key, value]) => key.startsWith("_") && key !== "_comment" && value && typeof value === "object")
  .map(([key, value]) => `${key}: ${Object.keys(value).length}`)
  .join(", ");
const topLevel = Object.entries(result).filter(([key, value]) => !key.startsWith("_") && typeof value === "string").length;
console.log(`wrote overrides/${to}.json (${sections}; ${topLevel} game entries; gap fills: ${filled.ui} UI / ${filled.game} game)`);
