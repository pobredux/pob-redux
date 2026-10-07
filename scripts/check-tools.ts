/**
 * Guard the chat panel's tool lists against the live registry. Needs the app
 * running with the MCP server on (`POB_REDUX_MCP=7315 bun run tauri dev`).
 *
 *   bun run check:tools
 *
 * Catches the three ways the lists in `src/lib/ai/allowed.ts` rot: a CORE name
 * that is not in ALLOWED (loaded never, silently), an ALLOWED name that no
 * longer exists in Rust, and a non-core tool find_tools cannot reach.
 */
import { readFileSync } from "node:fs";
import { findTools } from "../src/lib/ai/tools";
import { listTools, toDefs } from "./mcp-tools";

const { ALLOWED, CORE } = await import("../src/lib/ai/allowed");
const tools = await listTools();
const defs = toDefs(tools);

let failed = 0;
const check = (ok: boolean, good: string, bad: string) => {
  console.log(ok ? `ok   ${good}` : `FAIL ${bad}`);
  if (!ok) failed++;
};

const notAllowed = [...CORE].filter((n) => !ALLOWED.has(n));
check(!notAllowed.length, "CORE is a subset of ALLOWED", `CORE names not in ALLOWED: ${notAllowed.join(", ")}`);

// The live registry is the open game's; a name only the other game has is still declared in tools.rs.
const declared = new Set(
  [...readFileSync(new URL("../src-tauri/src/tools.rs", import.meta.url), "utf8").matchAll(/\b(?:ro|rw|del)\(\s*"([a-z_0-9]+)"|\bname: "([a-z_0-9]+)"/g)].map((m) => m[1] ?? m[2]),
);
const missing = [...ALLOWED].filter((n) => !tools.some((t) => t.name === n));
const gone = missing.filter((n) => !declared.has(n));
check(!gone.length, "every ALLOWED name exists in the registry", `ALLOWED names with no tool: ${gone.join(", ")}`);
if (missing.length > gone.length) console.log(`     (other game only: ${missing.filter((n) => declared.has(n)).join(", ")})`);

const unreachable = defs.filter((d) => !CORE.has(d.name) && !findTools(defs, d.name).some((f) => f.name === d.name));
check(!unreachable.length, "every non-core tool is reachable by name", `unreachable by name: ${unreachable.map((d) => d.name).join(", ")}`);

// The tools the instructions name by hand, reached the way the model would.
const intents: [string, string][] = [
  ["improve my gear", "optimise_gear"],
  ["which unique jewel should I use", "suggest_unique_jewels"],
  ["best passive nodes for life", "tree_suggest"],
  ["list the gems I could socket", "list_gems"],
  ["what supports work with this skill", "list_valid_supports"],
  ["craft a rare ring", "craft_rare"],
  ["equip this unique item text", "equip_item_raw"],
  ["add a gem to the group", "add_gem"],
  ["find an item in the database", "search_item_db"],
  ["undo the last tree change", "tree_undo"],
  ["find a timeless jewel seed", "search_timeless_seeds"],
];
for (const [query, want] of intents) {
  if (!defs.some((d) => d.name === want) && !CORE.has(want)) continue;
  const got = findTools(defs, query);
  check(got.some((d) => d.name === want), `"${query}" finds ${want}`, `"${query}" missed ${want}, got: ${got.map((d) => d.name).join(", ") || "nothing"}`);
}

// Ask mode drops every write. What is left has to still answer a question.
const askCore = defs.filter((d) => CORE.has(d.name) && d.read_only).map((d) => d.name);
for (const need of ["build_summary", "sanity_check", "library", "get_stats", "get_items", "get_skills", "search_tree"]) {
  check(askCore.includes(need), `Ask mode keeps ${need}`, `Ask mode lost ${need}: it is not both CORE and read-only`);
}
const askWrites = defs.filter((d) => CORE.has(d.name) && !d.read_only && d.name !== "checkpoint");
console.log(`ok   Ask mode: ${askCore.length} read tools, ${askWrites.length} writes dropped`);

const size = (names: Set<string>) =>
  Math.round(defs.filter((d) => names.has(d.name)).reduce((n, d) => n + d.name.length + d.description.length + 200, 0) / 4);
const all = Math.round(defs.reduce((n, d) => n + d.name.length + d.description.length + 200, 0) / 4);
console.log(`\n${defs.length} tools allowed, ${CORE.size} core`);
console.log(`definitions per turn: ~${size(CORE)} tokens core, ~${all} tokens if all were loaded`);

process.exit(failed ? 1 : 0);
