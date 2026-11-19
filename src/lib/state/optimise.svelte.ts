import {
  engine,
  powerScanParallel,
  planPointsParallel,
  type PointPlan,
  type PowerReportRow,
  type GearOptParams,
  type GearOptProgress,
  type GearOptResult,
  type GearProposal,
  type ClusterSuggestion,
  type ClusterSuggestions,
} from "$lib/engine.svelte";
import { build } from "$lib/state/build.svelte";

export type GearAim = "balanced" | "defence" | "damage";

/** The Optimize tab's inputs and results, kept while the user visits other tabs and dropped when another build opens. */
class OptimiseStore {
  preset = $state<GearAim>("balanced");
  itemLevel = $state(82);
  itemLevelTouched = $state(false);
  range = $state(1);
  chosen = $state<Set<string>>(new Set());
  running = $state(false);
  progress = $state<GearOptProgress | null>(null);
  result = $state<GearOptResult | null>(null);
  error = $state<string | null>(null);
  applied = $state<Set<string>>(new Set());
  cancel = false;

  clusterRunning = $state(false);
  clusters = $state<ClusterSuggestions | null>(null);
  clusterError = $state<string | null>(null);
  clusterRev = $state(-1);

  treeStat = $state("Life");
  treeStatChosen = false;
  notablesOnly = $state(true);
  treeRunning = $state(false);
  treeRows = $state<{ best: PowerReportRow[]; weakest: PowerReportRow[]; stat: string; ms: number } | null>(null);
  treeError = $state<string | null>(null);
  treeStale = $state(false);

  planBudget = $state<number | null>(null);
  planRunning = $state(false);
  plan = $state<PointPlan | null>(null);
  planError = $state<string | null>(null);
  planStale = $state(false);

  private generation: number | undefined;

  get clusterStale() {
    return this.clusters != null && build.rev !== this.clusterRev;
  }

  syncBuild = () => {
    const generation = build.info?.generation;
    if (generation === this.generation) return;
    const first = this.generation === undefined;
    this.generation = generation;
    if (first) return;
    this.cancel = true;
    this.itemLevelTouched = false;
    this.chosen = new Set();
    this.result = null;
    this.error = null;
    this.applied = new Set();
    this.clusters = null;
    this.clusterError = null;
    this.treeStatChosen = false;
    this.treeRows = null;
    this.treeError = null;
    this.treeStale = false;
    this.planBudget = null;
    this.plan = null;
    this.planError = null;
    this.planStale = false;
  };

  toggleSlot = (slot: string) => {
    const next = new Set(this.chosen);
    if (next.has(slot)) next.delete(slot);
    else next.add(slot);
    this.chosen = next;
  };

  run = async () => {
    if (this.running || !build.loaded) return;
    this.running = true;
    this.cancel = false;
    this.error = null;
    this.result = null;
    this.applied = new Set();
    const params: GearOptParams = { preset: this.preset, itemLevel: this.itemLevel, range: this.range, slots: [...this.chosen] };
    try {
      let r = await engine.gearOptStart(params);
      this.progress = r.progress;
      while (!r.done && !this.cancel) {
        r = await engine.gearOptStep(120);
        this.progress = r.progress;
        await new Promise((res) => setTimeout(res, 0));
      }
      if (!this.cancel) {
        this.result = await engine.gearOptResult();
        await build.sync();
      }
    } catch (e) {
      this.error = String(e);
    } finally {
      this.running = false;
      this.progress = null;
    }
  };

  apply = async (p: GearProposal) => {
    const ok = await build.run(() => engine.equipItemRaw(p.raw, p.slot));
    if (ok) this.applied = new Set([...this.applied, p.slot]);
  };

  applyAll = async () => {
    if (!this.result) return;
    for (const p of this.result.proposals) {
      if (!this.applied.has(p.slot)) await this.apply(p);
    }
  };

  findClusters = async () => {
    if (this.clusterRunning || !build.loaded) return;
    this.clusterRunning = true;
    this.clusterError = null;
    try {
      const r = await engine.suggestClusterJewels(this.preset, 8);
      await build.sync();
      this.clusters = r;
      this.clusterRev = build.rev;
    } catch (e) {
      this.clusterError = String(e);
    } finally {
      this.clusterRunning = false;
    }
  };

  applyCluster = async (s: ClusterSuggestion) => {
    await build.run(() => engine.applyClusterJewel(s));
  };

  scanTree = async () => {
    if (this.treeRunning || !build.loaded) return;
    this.treeRunning = true;
    this.treeError = null;
    try {
      const { result } = await powerScanParallel(this.treeStat, null);
      const best = result.report
        .filter((r) => !r.allocated && r.pathPower > 0 && (r.pathDist ?? 99) <= 8 && r.type !== "Mastery" && (!this.notablesOnly || r.type !== "Normal"))
        .sort((a, b) => b.pathPower - a.pathPower)
        .slice(0, 12);
      const weakest = result.report
        .filter((r) => r.allocated && r.type !== "ClassStart")
        .sort((a, b) => b.power - a.power)
        .slice(0, 8);
      this.treeRows = { best, weakest, stat: result.label, ms: result.ms };
      this.treeStale = false;
    } catch (e) {
      this.treeError = String(e);
    } finally {
      this.treeRunning = false;
    }
  };

  allocate = async (id: number) => {
    await build.allocNode(id);
    this.treeStale = this.planStale = true;
  };

  remove = async (id: number) => {
    await build.deallocNode(id);
    this.treeStale = this.planStale = true;
  };

  planTree = async (unspent: number) => {
    if (this.planRunning || !build.loaded) return;
    const budget = Math.round(this.planBudget ?? (unspent || 10));
    if (!(budget >= 1)) return;
    this.planRunning = true;
    this.planError = null;
    try {
      this.plan = (await planPointsParallel(this.treeStat, Math.min(budget, 120))).result;
      this.planStale = false;
    } catch (e) {
      this.planError = String(e);
    } finally {
      this.planRunning = false;
    }
  };

  applyPlan = async () => {
    if (!this.plan) return;
    for (const p of this.plan.picks) {
      if (!(await build.allocNode(p.id))) break;
    }
    this.planStale = this.treeStale = true;
  };
}

export const optimise = new OptimiseStore();
