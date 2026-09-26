import { invoke } from "@tauri-apps/api/core";
import { jsonSchema, tool, type ToolSet } from "ai";

import { ALLOWED, CORE } from "$lib/ai/allowed";

export { ALLOWED, CORE };

export interface ToolDef {
  name: string;
  description: string;
  schema: Record<string, unknown>;
  output_schema?: Record<string, unknown> | null;
  read_only: boolean;
  destructive: boolean;
  idempotent: boolean;
  open_world: boolean;
  slow: boolean;
}

/** Loads the rest of the registry on demand; handled in the loop, not in Rust. */
export const FIND_TOOLS = "find_tools";

export const FIND_TOOLS_DEF: ToolDef = {
  name: FIND_TOOLS,
  description:
    "Load more tools. Only some of the registry is loaded at the start of a conversation; anything named in the instructions that you cannot see is reachable here. `query` is what you want to do (\"craft a ring\", \"list gems\", \"rename the tree\") or an exact tool name. Returns the matches with their descriptions and makes them callable from the next step on.",
  schema: {
    type: "object",
    properties: {
      query: { type: "string", description: "What you want to do, or a tool name" },
    },
    required: ["query"],
  },
  read_only: true,
  destructive: false,
  idempotent: true,
  open_world: false,
  slow: false,
};

export async function loadToolDefs(): Promise<ToolDef[]> {
  const all = await invoke<ToolDef[]>("ai_tools");
  return all.filter((d) => ALLOWED.has(d.name));
}

/**
 * AI SDK tools with no `execute`. The loop runs them itself so a write can be
 * held for approval first — an `execute` callback would fire before the user
 * could decline.
 */
export function toToolSet(defs: ToolDef[]): ToolSet {
  return Object.fromEntries(
    defs.map((d) => [
      d.name,
      tool({
        description: d.description,
        inputSchema: jsonSchema(d.schema as never),
      }),
    ]),
  );
}

/** Score a tool against a free-text query; 0 means no match. */
function score(def: ToolDef, terms: string[]): number {
  const name = def.name.toLowerCase();
  const description = def.description.toLowerCase();
  let n = 0;
  for (const t of terms) {
    if (name === t) n += 100;
    else if (name.includes(t)) n += 10;
    else if (description.includes(t)) n += 1;
  }
  return n;
}

export function findTools(defs: ToolDef[], query: string, limit = 8): ToolDef[] {
  const terms = query
    .toLowerCase()
    .split(/[^a-z0-9_]+/)
    .filter((t) => t.length > 2);
  if (!terms.length) return [];
  return defs
    .map((d) => ({ d, n: score(d, terms) }))
    .filter((x) => x.n > 0)
    .sort((a, b) => b.n - a.n)
    .slice(0, limit)
    .map((x) => x.d);
}

export function callTool(name: string, args: unknown): Promise<unknown> {
  return invoke("ai_call_tool", { name, args: args ?? {} });
}
