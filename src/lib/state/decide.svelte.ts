import { clearKey, setKey } from "$lib/ai/providers";
import { decide, decideConfigure, decideSelect, decideStatus, type BackendStatus, type DecideStatus } from "$lib/ai/decide";
import { m } from "$lib/paraglide/messages";

const KEY = "pob-redux:experimental";

class DecideStore {
  status = $state<DecideStatus | null>(null);
  enabled = $state(false);
  routing = $state(true);
  busy = $state(false);
  testing = $state(false);
  error = $state<string | null>(null);
  test = $state<{ ok: boolean; text: string } | null>(null);
  private started = false;

  get current(): BackendStatus | undefined {
    return this.status?.backends.find((b) => b.id === this.status?.backend);
  }

  get ready() {
    return this.current?.ready ?? false;
  }

  get routingOn() {
    return this.enabled && this.routing && this.ready;
  }

  async init() {
    if (this.started) return;
    this.started = true;
    try {
      const saved = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      if (typeof saved.enabled === "boolean") this.enabled = saved.enabled;
      if (typeof saved.routing === "boolean") this.routing = saved.routing;
    } catch {}
    await this.refresh();
  }

  async refresh() {
    this.status = await decideStatus().catch(() => null);
  }

  private persist() {
    try {
      localStorage.setItem(KEY, JSON.stringify({ enabled: this.enabled, routing: this.routing }));
    } catch {}
  }

  setEnabled(on: boolean) {
    this.enabled = on;
    this.persist();
  }

  setRouting(on: boolean) {
    this.routing = on;
    this.persist();
  }

  private async act(fn: () => Promise<unknown>) {
    this.busy = true;
    this.error = null;
    this.test = null;
    try {
      await fn();
      await this.refresh();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  select = (id: string) => this.act(() => decideSelect(id));
  saveKey = (id: string, key: string) => this.act(() => setKey(id, key));
  removeKey = (id: string) => this.act(() => clearKey(id));
  configure = (id: string, base: string, model: string, account?: string) => this.act(() => decideConfigure(id, base, model, account));

  async runTest() {
    this.testing = true;
    this.error = null;
    this.test = null;
    try {
      const r = await decide(
        "How do I get more life on my Titan without losing damage?",
        { build: { type: "noul", instructions: "Is this a question about a Path of Exile character build?" } },
        15_000,
      );
      const a = r.answers.build;
      const p = a?.type === "noul" ? a.noul : NaN;
      this.test = { ok: true, text: m.experimental_test_ok({ model: r.model, ms: r.elapsed_ms, p: Number.isFinite(p) ? p.toFixed(2) : "?" }) };
    } catch (e) {
      this.test = { ok: false, text: String(e) };
    } finally {
      this.testing = false;
    }
  }
}

export const decider = new DecideStore();
