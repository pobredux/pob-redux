/**
 * Benchmark local Ollama models on the assistant's job: the same system prompt
 * and tool list the panel sends, the same manual tool loop, against the build
 * open in the app, with the tools executed through the MCP server.
 *
 *   bun scripts/bench-models.ts [--models a,b,c] [--prompts 1,2,3] [--out target/bench]
 *
 * Needs the app running with POB_REDUX_MCP=7315 and Ollama on localhost.
 * Every run reloads the planner build first, so prompts that change the build
 * start from the same state. Scores are deterministic checks against facts of
 * that build, not a model-judged rubric.
 */
import { mkdirSync, writeFileSync } from "node:fs";

import { ALLOWED } from "../src/lib/ai/allowed";
import { style } from "../src/lib/ai/prompt";

const STYLE = style("poe2");

const MCP = "http://127.0.0.1:7315/mcp";
const OLLAMA = "http://localhost:11434";
const BUILD = "C:/Users/CKH/Documents/My Games/Path of Exile 2/BuildPlanner/Live - Slam Titan (Leveling & Endgame).build";
const NUM_CTX = 32768;
const MAX_STEPS = 16;
const REQUEST_TIMEOUT_MS = 300_000;
const RUN_TIMEOUT_MS = 12 * 60_000;

// ---------------------------------------------------------------------------
// MCP client
// ---------------------------------------------------------------------------

let session: string | null = null;
let rpcId = 0;

async function rpc(method: string, params: unknown = {}): Promise<any> {
  const headers: Record<string, string> = { "content-type": "application/json", accept: "application/json, text/event-stream" };
  if (session) headers["mcp-session-id"] = session;
  const res = await fetch(MCP, { method: "POST", headers, body: JSON.stringify({ jsonrpc: "2.0", id: ++rpcId, method, params }) });
  const sid = res.headers.get("mcp-session-id");
  if (sid) session = sid;
  const text = await res.text();
  if (res.headers.get("content-type")?.includes("text/event-stream")) {
    const lines = text.split("\n").filter((l) => l.startsWith("data:"));
    return JSON.parse(lines[lines.length - 1].slice(5));
  }
  return text ? JSON.parse(text) : null;
}

async function mcpInit(): Promise<{ instructions: string; tools: any[] }> {
  const init = await rpc("initialize", { protocolVersion: "2025-03-26", capabilities: {}, clientInfo: { name: "bench", version: "0" } });
  await fetch(MCP, {
    method: "POST",
    headers: { "content-type": "application/json", "mcp-session-id": session!, accept: "application/json, text/event-stream" },
    body: JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }),
  });
  const list = await rpc("tools/list", {});
  const tools = (list.result?.tools ?? []).filter((t: any) => ALLOWED.has(t.name));
  return { instructions: init.result?.instructions ?? "", tools };
}

async function callTool(name: string, args: unknown): Promise<{ text: string; isError: boolean }> {
  const r = await rpc("tools/call", { name, arguments: args ?? {} });
  if (r?.error) return { text: `Error: ${r.error.message ?? JSON.stringify(r.error)}`, isError: true };
  const text = r?.result?.content?.map((c: any) => c.text ?? "").join("\n") ?? "";
  return { text, isError: r?.result?.isError === true };
}

async function toolJson(name: string, args: unknown = {}): Promise<any> {
  const r = await callTool(name, args);
  try {
    return JSON.parse(r.text);
  } catch {
    return null;
  }
}

async function resetBuild() {
  await callTool("load_build", { source: BUILD });
}

/** The same per-turn snapshot the panel prepends to the user message. */
async function buildContext(): Promise<string> {
  const ch = await toolJson("get_character");
  const side = await toolJson("get_sidebar");
  const rows = (side?.rows ?? [])
    .slice(0, 24)
    .map((r: any) => `${r.label ?? ""} ${r.value ?? ""}`.trim())
    .filter((s: string) => s.length > 1)
    .join("; ");
  const asc = ch?.ascendClassName && ch.ascendClassName !== "None" ? ` (${ch.ascendClassName})` : "";
  return `Open build: ${ch?.name ?? "unnamed"} — level ${ch?.level} ${ch?.className}${asc}\nSidebar: ${rows}`;
}

// ---------------------------------------------------------------------------
// Ollama
// ---------------------------------------------------------------------------

interface OllamaReply {
  message: { role: string; content: string; thinking?: string; tool_calls?: { function: { name: string; arguments: any } }[] };
  done_reason?: string;
  prompt_eval_count?: number;
  eval_count?: number;
  total_duration?: number;
  prompt_eval_duration?: number;
  eval_duration?: number;
  error?: string;
}

async function chat(model: string, messages: any[], tools: any[]): Promise<OllamaReply> {
  const res = await fetch(`${OLLAMA}/api/chat`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ model, messages, tools, stream: false, keep_alive: "15m", options: { num_ctx: NUM_CTX, temperature: 0 } }),
    signal: AbortSignal.timeout(REQUEST_TIMEOUT_MS),
  });
  const j = (await res.json()) as OllamaReply;
  if (!res.ok || j.error) throw new Error(j.error ?? `HTTP ${res.status}`);
  return j;
}

// ---------------------------------------------------------------------------
// Prompts and scoring
// ---------------------------------------------------------------------------

interface Run {
  model: string;
  prompt: number;
  title: string;
  ended: "stop" | "cap" | "error" | "timeout" | "no-tools";
  steps: number;
  toolCalls: number;
  toolErrors: number;
  unknownTools: number;
  argErrors: number;
  wallMs: number;
  promptTokens: number;
  evalTokens: number;
  evalTokPerSec: number | null;
  words: number;
  dashes: number;
  narration: number;
  score: number;
  notes: string;
  answer: string;
  calls: { name: string; args: any; error?: string }[];
  error?: string;
}

interface Prompt {
  id: number;
  title: string;
  text: string;
  /** 0..1 from the final answer and the calls made; state checks may query MCP. */
  score: (answer: string, calls: Run["calls"]) => Promise<{ score: number; notes: string }>;
}

const has = (s: string, re: RegExp) => re.test(s);
const called = (calls: Run["calls"], name: string) => calls.some((c) => c.name === name && !c.error);

const PROMPTS: Prompt[] = [
  {
    id: 1,
    title: "life and resists",
    text: "What is my total life, and are my elemental resistances capped?",
    async score(a, calls) {
      let s = 0;
      const notes: string[] = [];
      if (has(a, /2[,.]?451/)) s += 0.4;
      else notes.push("life 2,451 missing");
      if (has(a, /\b64\b/)) s += 0.3;
      else notes.push("lightning 64 missing");
      if (has(a, /\b(not|aren't|isn't|below|under|uncapped)\b/i)) s += 0.2;
      else notes.push("did not say uncapped");
      if (calls.some((c) => !c.error)) s += 0.1;
      else notes.push("no successful tool call");
      return { score: s, notes: notes.join("; ") };
    },
  },
  {
    id: 2,
    title: "buttons",
    text: "Which of my skills need a keypress? Name each one.",
    async score(a) {
      const names = ["Mace Strike", "Stampede", "Forge Hammer", "Sunder", "Hammer of the Gods", "Ancestral Cry", "Infernal Cry"];
      const hit = names.filter((n) => a.includes(n)).length;
      const wrong = ["Herald of Ash", "Scavenged Plating", "Overwhelming Presence"].filter((n) => new RegExp(`${n}[^.]{0,40}(press|keypress|button)`, "i").test(a) || new RegExp(`(press|keypress|button)[^.]{0,60}${n}`, "i").test(a)).length;
      const notes: string[] = [];
      if (hit < 7) notes.push(`${hit}/7 buttons named`);
      if (wrong) notes.push(`${wrong} always-on gems called buttons`);
      return { score: Math.max(0, hit / 7 - wrong * 0.15), notes: notes.join("; ") };
    },
  },
  {
    id: 3,
    title: "Hammer of the Gods condition",
    text: "What does Hammer of the Gods need before it can be used?",
    async score(a, calls) {
      let s = 0;
      const notes: string[] = [];
      if (has(a, /glory/i)) s += 0.6;
      else notes.push("Glory missing");
      if (has(a, /heavy stun|stun/i)) s += 0.2;
      else notes.push("stun missing");
      if (called(calls, "skill_info")) s += 0.2;
      else notes.push("skill_info not used");
      return { score: s, notes: notes.join("; ") };
    },
  },
  {
    id: 4,
    title: "best life notable",
    text: "Which unallocated notable would add the most life per point? Do not change anything.",
    async score(a, calls) {
      let s = 0;
      const notes: string[] = [];
      if (has(a, /\bBeef\b/)) s += 0.7;
      else notes.push("Beef missing");
      if (called(calls, "tree_suggest")) s += 0.2;
      else notes.push("tree_suggest not used");
      const wrote = calls.some((c) => ["alloc_node", "alloc_path", "dealloc_node"].includes(c.name) && !c.error);
      if (!wrote) s += 0.1;
      else notes.push("changed the tree");
      return { score: s, notes: notes.join("; ") };
    },
  },
  {
    id: 5,
    title: "improve boots and belt",
    text: "Improve my boots and belt for defence. Do not touch any other slot.",
    async score(a, calls) {
      let s = 0;
      const notes: string[] = [];
      const opt = calls.find((c) => c.name === "optimise_gear" && !c.error);
      const craft = calls.filter((c) => c.name === "craft_rare" && !c.error);
      if (opt) s += 0.5;
      else if (craft.length) {
        s += 0.3;
        notes.push("crafted by hand");
      } else notes.push("no gear tool used");
      const items = await toolJson("get_items");
      const slots: any[] = items?.slots ?? [];
      const changed = (slot: string) => {
        const row = slots.find((x) => x.slot === slot);
        return row && typeof row.itemName === "string" && !row.itemName.startsWith("Imported ");
      };
      const bootsChanged = changed("Boots");
      const beltChanged = changed("Belt");
      const others = ["Weapon 1", "Helmet", "Body Armour", "Gloves", "Amulet", "Ring 1", "Ring 2"].filter(changed);
      if (bootsChanged && beltChanged) s += 0.3;
      else if (bootsChanged || beltChanged) {
        s += 0.15;
        notes.push("only one slot applied");
      } else notes.push("nothing applied");
      if (others.length) {
        s -= 0.3;
        notes.push(`touched ${others.join(", ")}`);
      }
      if (has(a, /\d/)) s += 0.1;
      else notes.push("no numbers in answer");
      if (opt && Array.isArray(opt.args?.slots) && opt.args.slots.length && !opt.args.slots.every((x: string) => /boots|belt/i.test(x))) {
        notes.push("asked the optimiser for other slots");
        s -= 0.1;
      }
      return { score: Math.max(0, Math.min(1, s)), notes: notes.join("; ") };
    },
  },
  {
    id: 6,
    title: "remove Infernal Cry",
    text: "Remove the Infernal Cry skill setup completely.",
    async score(a, calls) {
      let s = 0;
      const notes: string[] = [];
      const skills = await toolJson("get_skills");
      const groups: any[] = skills?.socketGroups ?? [];
      const still = groups.some((g) => (g.gems ?? []).some((gem: any) => gem.name === "Infernal Cry"));
      const disabled = groups.some((g) => g.enabled === false && (g.gems ?? []).some((gem: any) => gem.name === "Infernal Cry"));
      if (!still) s += 0.7;
      else if (disabled) {
        s += 0.3;
        notes.push("disabled, not removed");
      } else notes.push("still present");
      const removed = calls.filter((c) => c.name === "remove_socket_group" && !c.error).length;
      if (removed > 1) {
        s -= 0.4;
        notes.push(`removed ${removed} groups`);
      }
      const lost = ["Hammer of the Gods", "Sunder", "Stampede", "Mace Strike", "Forge Hammer", "Ancestral Cry", "Herald of Ash"].filter((n) => !groups.some((g) => (g.gems ?? []).some((gem: any) => gem.name === n)));
      if (lost.length) {
        s -= 0.5;
        notes.push(`lost ${lost.join(", ")}`);
      }
      if (called(calls, "skill_info")) s += 0.1;
      if (has(a, /\d|removed|gone|deleted/i)) s += 0.2;
      else notes.push("did not confirm");
      return { score: Math.max(0, Math.min(1, s)), notes: notes.join("; ") };
    },
  },
];

// ---------------------------------------------------------------------------
// The loop
// ---------------------------------------------------------------------------

function toOllamaTools(tools: any[]) {
  return tools.map((t) => ({ type: "function", function: { name: t.name, description: t.description, parameters: t.inputSchema ?? { type: "object", properties: {} } } }));
}

async function runOne(model: string, prompt: Prompt, instructions: string, tools: any[]): Promise<Run> {
  await resetBuild();
  const context = await buildContext();
  const byName = new Map(tools.map((t) => [t.name, t]));
  const otools = toOllamaTools(tools);
  const messages: any[] = [
    { role: "system", content: instructions + STYLE },
    { role: "user", content: `${context}\n\n${prompt.text}` },
  ];
  const run: Run = {
    model,
    prompt: prompt.id,
    title: prompt.title,
    ended: "stop",
    steps: 0,
    toolCalls: 0,
    toolErrors: 0,
    unknownTools: 0,
    argErrors: 0,
    wallMs: 0,
    promptTokens: 0,
    evalTokens: 0,
    evalTokPerSec: null,
    words: 0,
    dashes: 0,
    narration: 0,
    score: 0,
    notes: "",
    answer: "",
    calls: [],
  };
  const t0 = Date.now();
  let evalNs = 0;
  let final = "";
  try {
    for (let step = 0; step < MAX_STEPS; step++) {
      if (Date.now() - t0 > RUN_TIMEOUT_MS) {
        run.ended = "timeout";
        break;
      }
      const reply = await chat(model, messages, otools);
      run.steps++;
      run.promptTokens += reply.prompt_eval_count ?? 0;
      run.evalTokens += reply.eval_count ?? 0;
      evalNs += reply.eval_duration ?? 0;
      const msg = reply.message;
      // Thinking models need their reasoning back in the history, or the
      // final turn after a tool result comes out empty.
      messages.push({ role: "assistant", content: msg.content ?? "", tool_calls: msg.tool_calls, ...(msg.thinking ? { thinking: msg.thinking } : {}) });
      const calls = msg.tool_calls ?? [];
      if (!calls.length) {
        final = msg.content ?? "";
        run.ended = run.steps === 1 ? "no-tools" : "stop";
        break;
      }
      if (msg.content && msg.content.trim()) run.narration++;
      for (const c of calls) {
        const name = c.function?.name ?? "";
        let args = c.function?.arguments ?? {};
        if (typeof args === "string") {
          try {
            args = JSON.parse(args);
          } catch {
            run.argErrors++;
            args = {};
          }
        }
        run.toolCalls++;
        const def = byName.get(name);
        let text: string;
        if (!def) {
          run.unknownTools++;
          text = `Error: unknown tool ${name}`;
          run.calls.push({ name, args, error: "unknown tool" });
        } else {
          const required: string[] = def.inputSchema?.required ?? [];
          const missing = required.filter((k) => args[k] === undefined || args[k] === null);
          if (missing.length) run.argErrors++;
          const r = await callTool(name, args);
          text = r.text;
          if (r.isError) run.toolErrors++;
          run.calls.push({ name, args, error: r.isError ? text.slice(0, 200) : undefined });
        }
        messages.push({ role: "tool", content: text.length > 12000 ? text.slice(0, 12000) + " …(truncated)" : text, tool_name: name });
      }
      if (step === MAX_STEPS - 1) run.ended = "cap";
    }
  } catch (e) {
    run.ended = String(e).includes("TimeoutError") || String(e).includes("timed out") ? "timeout" : "error";
    run.error = String(e).slice(0, 300);
  }
  run.wallMs = Date.now() - t0;
  run.evalTokPerSec = evalNs > 0 ? Math.round((run.evalTokens / (evalNs / 1e9)) * 10) / 10 : null;
  run.answer = final.trim();
  run.words = run.answer ? run.answer.split(/\s+/).length : 0;
  run.dashes = (run.answer.match(/[—–]/g) ?? []).length;
  const sc = await prompt.score(run.answer, run.calls);
  run.score = Math.round(sc.score * 100) / 100;
  run.notes = sc.notes;
  return run;
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

function arg(name: string): string | undefined {
  const i = process.argv.indexOf(`--${name}`);
  return i >= 0 ? process.argv[i + 1] : undefined;
}

async function listModels(): Promise<{ name: string; size: number }[]> {
  const r = await fetch(`${OLLAMA}/api/tags`).then((x) => x.json());
  return (r.models ?? []).map((m: any) => ({ name: m.name, size: m.size }));
}

async function main() {
  const out = arg("out") ?? "target/bench";
  mkdirSync(out, { recursive: true });
  const stamp = new Date().toISOString().replace(/[:.]/g, "-").slice(0, 19);
  const all = await listModels();
  const wanted = arg("models")?.split(",").map((s) => s.trim()).filter(Boolean) ?? all.map((m) => m.name);
  const promptIds = arg("prompts")?.split(",").map(Number) ?? PROMPTS.map((p) => p.id);
  const prompts = PROMPTS.filter((p) => promptIds.includes(p.id));
  const { instructions, tools } = await mcpInit();
  console.log(`tools: ${tools.length}, system prompt: ${(instructions + STYLE).length} chars, models: ${wanted.join(", ")}`);
  const runs: Run[] = [];
  const file = `${out}/${stamp}.json`;
  for (const model of wanted) {
    for (const p of prompts) {
      const r = await runOne(model, p, instructions, tools);
      runs.push(r);
      const line = `${model} | P${p.id} ${p.title} | score ${r.score} | ${r.ended} | steps ${r.steps} calls ${r.toolCalls} err ${r.toolErrors + r.unknownTools + r.argErrors} | ${Math.round(r.wallMs / 1000)}s | ${r.words}w ${r.dashes}d | ${r.notes}${r.error ? " | " + r.error : ""}`;
      console.log(line);
      writeFileSync(file, JSON.stringify({ numCtx: NUM_CTX, models: all, runs }, null, 2));
    }
  }
  // Summary table
  const lines: string[] = ["| Model | Size | Score | Runs OK | Tool errors | Avg steps | Avg time | tok/s | Avg words | Dashes |", "|---|---|---|---|---|---|---|---|---|---|"];
  const byModel = new Map<string, Run[]>();
  for (const r of runs) byModel.set(r.model, [...(byModel.get(r.model) ?? []), r]);
  const rows = [...byModel.entries()].map(([model, rs]) => {
    const size = all.find((m) => m.name === model)?.size ?? 0;
    const score = rs.reduce((a, r) => a + r.score, 0) / rs.length;
    const ok = rs.filter((r) => r.ended === "stop").length;
    const errs = rs.reduce((a, r) => a + r.toolErrors + r.unknownTools + r.argErrors, 0);
    const steps = rs.reduce((a, r) => a + r.steps, 0) / rs.length;
    const time = rs.reduce((a, r) => a + r.wallMs, 0) / rs.length / 1000;
    const tps = rs.filter((r) => r.evalTokPerSec).map((r) => r.evalTokPerSec!);
    const tp = tps.length ? tps.reduce((a, b) => a + b, 0) / tps.length : 0;
    const words = rs.reduce((a, r) => a + r.words, 0) / rs.length;
    const dashes = rs.reduce((a, r) => a + r.dashes, 0);
    return { model, size, score, ok, n: rs.length, errs, steps, time, tp, words, dashes };
  });
  rows.sort((a, b) => b.score - a.score);
  for (const r of rows) {
    lines.push(`| ${r.model} | ${(r.size / 1e9).toFixed(1)} GB | ${(r.score * 100).toFixed(0)}% | ${r.ok}/${r.n} | ${r.errs} | ${r.steps.toFixed(1)} | ${r.time.toFixed(0)}s | ${r.tp.toFixed(0)} | ${r.words.toFixed(0)} | ${r.dashes} |`);
  }
  const md = lines.join("\n");
  console.log("\n" + md);
  writeFileSync(`${out}/${stamp}.md`, md + "\n");
  console.log(`\nwrote ${file}`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
