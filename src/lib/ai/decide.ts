import { invoke } from "@tauri-apps/api/core";

import type { ToolDef } from "$lib/ai/tools";
import type { Game } from "$lib/state/game.svelte";

export interface BackendStatus {
  id: string;
  key_id: string;
  label: string;
  needs_key: boolean;
  keys_url: string | null;
  base_url: string;
  default_base: string;
  model: string;
  default_model: string;
  has_key: boolean;
  hint: string | null;
  ready: boolean;
}

export interface DecideStatus {
  backend: string;
  backends: BackendStatus[];
}

/** TypeSafe's System One question types; Kev serves the same shapes. */
export type Question =
  | { type: "noul"; instructions: string }
  | { type: "choice"; instructions: string; criteria: Record<string, string | null> }
  | { type: "score"; instructions: string; criteria: string[] };

export type Answer =
  | { type: "noul"; noul: number }
  | { type: "choice"; choice: string; confidence: number; probabilities: Record<string, number> }
  | { type: "score"; score: number; confidence: number; probabilities: Record<string, number> };

export interface Decision {
  model: string;
  backend: string;
  answers: Record<string, Answer | undefined>;
  usage?: { input_tokens: number; output_tokens: number };
  elapsed_ms: number;
}

export const decideStatus = () => invoke<DecideStatus>("decide_status");
export const decideSelect = (backend: string) => invoke<void>("decide_select", { backend });
export const decideConfigure = (backend: string, baseUrl: string, model: string) =>
  invoke<void>("decide_configure", { backend, baseUrl, model });
export const decide = (state: unknown, questions: Record<string, Question>, timeoutMs?: number) =>
  invoke<Decision>("decide_ask", { state, questions, timeoutMs });

const NONE = "none";

export function firstSentence(text: string): string {
  const s = text.split(/(?<=[.!?])\s/)[0] ?? text;
  return s.length > 200 ? `${s.slice(0, 197)}...` : s;
}

export type Pick = { name: string; p: number };

export interface Routed {
  picks: Pick[];
  model: string;
  ms: number;
}

export function routingRequest(request: string, candidates: ToolDef[], previous?: string, game: Game = "poe2") {
  const criteria: Record<string, string> = Object.fromEntries(candidates.map((d) => [d.name, firstSentence(d.description)]));
  criteria[NONE] =
    "None of these. The request needs only the build's stats, gear, skills, config or passive tree, which are already available, or no tool at all.";
  const state = previous ? { request, previous: previous.slice(0, 600) } : { request };
  const questions: Record<string, Question> = {
    tool: {
      type: "choice",
      instructions:
        `A Path of Exile ${game === "poe1" ? 1 : 2} player asks an assistant in a Path of Building app about their build. Which tool does the assistant need for \`request\`?` +
        (previous ? " `previous` is the player's earlier message, for context." : ""),
      criteria,
    },
  };
  return { state, questions };
}

export function readPicks(answers: Decision["answers"], min: number, max: number): Pick[] {
  const answer = answers.tool;
  const probabilities = answer?.type === "choice" ? answer.probabilities : {};
  return Object.entries(probabilities)
    .filter(([name, p]) => name !== NONE && p >= min)
    .sort((a, b) => b[1] - a[1])
    .slice(0, max)
    .map(([name, p]) => ({ name, p }));
}

export async function routeTools(
  request: string,
  candidates: ToolDef[],
  opts: { previous?: string; min: number; max: number; timeoutMs?: number; game?: Game },
): Promise<Routed> {
  if (!candidates.length) return { picks: [], model: "", ms: 0 };
  const { state, questions } = routingRequest(request, candidates, opts.previous, opts.game);
  const res = await decide(state, questions, opts.timeoutMs ?? 4000);
  const known = new Set(candidates.map((d) => d.name));
  const picks = readPicks(res.answers, opts.min, opts.max).filter((p) => known.has(p.name));
  return { picks, model: res.model, ms: res.elapsed_ms };
}
