<script lang="ts">
  import {
    engine,
    powerScanParallel,
    planPointsParallel,
    type PointPlan,
    type PowerReportRow,
    type BuildSummary,
    type GearOptParams,
    type GearOptProgress,
    type GearOptResult,
    type GearProposal,
    type SanityCheck,
    type SlotInfo,
  } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { stripPobText } from "$lib/pobtext";
  import { m } from "$lib/paraglide/messages";

  // Review: PoB's numbers plus the rule-based findings, refreshed with the build.
  let summary = $state<BuildSummary | null>(null);
  let sanity = $state<SanityCheck | null>(null);
  let slots = $state<SlotInfo[]>([]);
  let loadedRev = -1;

  $effect(() => {
    const rev = build.rev;
    if (!build.loaded || rev === loadedRev) return;
    loadedRev = rev;
    Promise.all([engine.buildSummary(), engine.sanityCheck(), engine.listSlots()])
      .then(([s, c, l]) => {
        summary = s;
        sanity = c;
        slots = l.slots.filter((x) => x.shown !== false && !x.inactive);
      })
      .catch(() => {});
  });

  // Gear optimiser
  const OPT_SLOTS = ["Weapon 1", "Weapon 2", "Helmet", "Body Armour", "Gloves", "Boots", "Belt", "Amulet", "Ring 1", "Ring 2"];
  let preset = $state<"balanced" | "defence" | "damage">("balanced");
  let itemLevel = $state(82);
  // Mods need an item level the character could have found, so the default
  // follows the level until the user types one.
  let itemLevelTouched = $state(false);
  $effect(() => {
    const lvl = summary?.characterLevel;
    if (lvl && !itemLevelTouched) itemLevel = Math.min(82, lvl);
  });
  let range = $state(1);
  let chosen = $state<Set<string>>(new Set());
  let running = $state(false);
  let progress = $state<GearOptProgress | null>(null);
  let result = $state<GearOptResult | null>(null);
  let error = $state<string | null>(null);
  let applied = $state<Set<string>>(new Set());
  let cancel = false;

  const equipped = $derived(
    slots.filter((s) => OPT_SLOTS.includes(s.slot) && s.itemId > 0).map((s) => ({ slot: s.slot, name: s.itemName, unique: s.itemRarity === "UNIQUE" })),
  );

  $effect(() => {
    // Default to every equipped rare slot; keep the user's picks across refreshes.
    const list = equipped;
    if (chosen.size === 0 && list.length) chosen = new Set(list.filter((e) => !e.unique).map((e) => e.slot));
  });

  function toggleSlot(slot: string) {
    const next = new Set(chosen);
    if (next.has(slot)) next.delete(slot);
    else next.add(slot);
    chosen = next;
  }

  async function run() {
    if (running || !build.loaded) return;
    running = true;
    cancel = false;
    error = null;
    result = null;
    applied = new Set();
    const params: GearOptParams = { preset, itemLevel, range, slots: [...chosen] };
    try {
      let r = await engine.gearOptStart(params);
      progress = r.progress;
      while (!r.done && !cancel) {
        r = await engine.gearOptStep(120);
        progress = r.progress;
        await new Promise((res) => setTimeout(res, 0));
      }
      if (!cancel) {
        result = await engine.gearOptResult();
        await build.sync();
      }
    } catch (e) {
      error = String(e);
    } finally {
      running = false;
      progress = null;
    }
  }

  async function apply(p: GearProposal) {
    const ok = await build.run(() => engine.equipItemRaw(p.raw, p.slot));
    if (ok) applied = new Set([...applied, p.slot]);
  }

  async function applyAll() {
    if (!result) return;
    for (const p of result.proposals) {
      if (!applied.has(p.slot)) await apply(p);
    }
  }

  const HEADLINE = $derived<{ key: string; label: string; pct?: boolean }[]>([
    { key: "Life", label: m.opt_stat_life() },
    { key: "TotalEHP", label: m.opt_stat_ehp() },
    { key: "Armour", label: m.opt_stat_armour() },
    { key: "CombinedDPS", label: m.opt_stat_dps() },
    { key: "FireResist", label: m.opt_stat_fire_res(), pct: true },
    { key: "ColdResist", label: m.opt_stat_cold_res(), pct: true },
    { key: "LightningResist", label: m.opt_stat_lightning_res(), pct: true },
    { key: "ChaosResist", label: m.opt_stat_chaos_res(), pct: true },
    { key: "Str", label: m.opt_stat_str() },
    { key: "Dex", label: m.opt_stat_dex() },
    { key: "Int", label: m.opt_stat_int() },
  ]);

  const fmt = (v: number | undefined, pct = false) =>
    v == null ? "" : pct ? `${Math.round(v)}%` : Math.round(v).toLocaleString();
  const fmtDelta = (v: number | undefined, pct = false) => {
    if (v == null || Math.abs(v) < 0.5) return "";
    const n = Math.round(v);
    return (n > 0 ? "+" : "") + (pct ? `${n}%` : n.toLocaleString());
  };
  const deltaClass = (v: number | undefined) => (v == null || Math.abs(v) < 0.5 ? "" : v > 0 ? "up" : "down");

  const DELTA_KEYS = $derived<{ key: string; label: string; pct?: boolean }[]>([
    { key: "Life", label: m.opt_delta_life() },
    { key: "TotalEHP", label: m.opt_delta_ehp() },
    { key: "CombinedDPS", label: m.opt_delta_dps() },
    { key: "Armour", label: m.opt_delta_armour() },
    { key: "FireResist", label: m.opt_delta_fire(), pct: true },
    { key: "ColdResist", label: m.opt_delta_cold(), pct: true },
    { key: "LightningResist", label: m.opt_delta_lightning(), pct: true },
    { key: "ChaosResist", label: m.opt_delta_chaos(), pct: true },
    { key: "Str", label: m.opt_delta_str() },
    { key: "Dex", label: m.opt_delta_dex() },
    { key: "Int", label: m.opt_delta_int() },
  ]);

  const sevClass: Record<string, string> = { high: "bad", medium: "warn", low: "low" };

  /** An implicit as a shopping-list line: the roll range dropped, one line only. */
  const plainImplicit = (s: string) =>
    s
      .split("\n")[0]
      .replace(/\{[^}]*\}/g, "")
      .replace(/\(?[\d.]+-[\d.]+\)?%?/g, "")
      .replace(/\s+/g, " ")
      .trim();

  // Tree: node power for one stat, the best unallocated per point and the
  // weakest allocated, straight from the scan the tree tab already runs.
  const TREE_STATS = $derived<[string, string][]>([
    ["Life", m.opt_stat_life()],
    ["EnergyShield", m.opt_tree_es()],
    ["TotalEHP", m.opt_tree_ehp()],
    ["CombinedDPS", m.opt_stat_dps()],
    ["Armour", m.opt_stat_armour()],
    ["EffectiveMovementSpeedMod", m.opt_tree_speed()],
  ]);
  let treeStat = $state("Life");
  let treeStatChosen = false;
  $effect(() => {
    if (!treeStatChosen && summary) treeStat = summary.energyShield > summary.life ? "EnergyShield" : "Life";
  });
  let notablesOnly = $state(true);
  let treeRunning = $state(false);
  let treeRows = $state<{ best: PowerReportRow[]; weakest: PowerReportRow[]; stat: string; ms: number } | null>(null);
  let treeError = $state<string | null>(null);

  async function scanTree() {
    if (treeRunning || !build.loaded) return;
    treeRunning = true;
    treeError = null;
    try {
      const { result } = await powerScanParallel(treeStat, null);
      const best = result.report
        .filter((r) => !r.allocated && r.pathPower > 0 && (r.pathDist ?? 99) <= 8 && r.type !== "Mastery" && (!notablesOnly || r.type !== "Normal"))
        .sort((a, b) => b.pathPower - a.pathPower)
        .slice(0, 12);
      const weakest = result.report
        .filter((r) => r.allocated && r.type !== "ClassStart")
        .sort((a, b) => b.power - a.power)
        .slice(0, 8);
      treeRows = { best, weakest, stat: result.label, ms: result.ms };
      treeStale = false;
    } catch (e) {
      treeError = String(e);
    } finally {
      treeRunning = false;
    }
  }

  // A change leaves the list in place but stale; a rescan costs seconds.
  let treeStale = $state(false);
  async function allocate(id: number) {
    await build.allocNode(id);
    treeStale = planStale = true;
  }
  async function remove(id: number) {
    await build.deallocNode(id);
    treeStale = planStale = true;
  }

  let planBudget = $state<number | null>(null);
  let planRunning = $state(false);
  let plan = $state<PointPlan | null>(null);
  let planError = $state<string | null>(null);
  let planStale = $state(false);
  const unspent = $derived(summary ? Math.max(0, summary.pointsAvailableMax - summary.passivePointsSpent) : 0);

  async function planTree() {
    if (planRunning || !build.loaded) return;
    const budget = Math.round(planBudget ?? (unspent || 10));
    if (!(budget >= 1)) return;
    planRunning = true;
    planError = null;
    try {
      plan = (await planPointsParallel(treeStat, Math.min(budget, 120))).result;
      planStale = false;
    } catch (e) {
      planError = String(e);
    } finally {
      planRunning = false;
    }
  }

  async function applyPlan() {
    if (!plan) return;
    for (const p of plan.picks) {
      if (!(await build.allocNode(p.id))) break;
    }
    planStale = treeStale = true;
  }
  const fmtGain = (v: number) => (Math.abs(v) >= 100 ? Math.round(v).toLocaleString() : (Math.round(v * 10) / 10).toString());
</script>

<div class="page">
  <p class="disclaimer"><strong>{m.opt_disclaimer_title()}</strong> {m.opt_disclaimer()}</p>
  <section class="col review">
    <div class="head">
      <span class="title">{m.opt_review()}</span>
      {#if sanity}
        <span class="dim small">
          <span class="mono">{sanity.high}</span> {m.opt_sev_high()} · <span class="mono">{sanity.medium}</span> {m.opt_sev_medium()} · <span class="mono">{sanity.low}</span> {m.opt_sev_low()}
        </span>
      {/if}
    </div>
    {#if !build.loaded}
      <div class="dim small pad">{m.opt_open_build()}</div>
    {:else if summary}
      <div class="facts">
        <div class="fact" title={m.opt_active_title()}><span class="k">{m.opt_active()}</span><span class="v mono">{summary.activeSkills}</span></div>
        <div class="fact" title={m.opt_automatic_title()}><span class="k">{m.opt_automatic()}</span><span class="v mono">{summary.persistentSkills + summary.triggerSkills + summary.metaSkills}</span></div>
        <div class="fact" title={m.opt_points_title({ level: summary.characterLevel })}><span class="k">{m.opt_points()}</span><span class="v mono">{summary.passivePointsSpent}<span class="dim"> / {summary.pointsAvailableMin === summary.pointsAvailableMax ? summary.pointsAvailableMax : `${summary.pointsAvailableMin}–${summary.pointsAvailableMax}`}</span></span></div>
        {#if game.isPoe2}
          <div class="fact" title={m.opt_spirit_title()}><span class="k">{m.opt_spirit()}</span><span class="v mono">{summary.spiritReserved}<span class="dim"> / {summary.spirit}</span></span></div>
          <div class="fact" title={m.opt_charms_title()}><span class="k">{m.opt_charms()}</span><span class="v mono">{summary.charmsEquipped}<span class="dim"> / {summary.charmLimit}</span></span></div>
        {/if}
        <div class="fact" title={m.opt_resists_title()}><span class="k">{m.opt_resists()}</span><span class="v mono"><span class="fire" class:bad={summary.fireResist < 75}>{Math.round(summary.fireResist)}</span> / <span class="cold" class:bad={summary.coldResist < 75}>{Math.round(summary.coldResist)}</span> / <span class="lightning" class:bad={summary.lightningResist < 75}>{Math.round(summary.lightningResist)}</span> / <span class="chaos">{Math.round(summary.chaosResist)}</span></span></div>
      </div>
      <div class="findings">
        {#if sanity && sanity.findings.length === 0}
          <div class="dim small pad">{m.opt_no_findings()}</div>
        {/if}
        {#each sanity?.findings ?? [] as f}
          <div class="finding">
            <span class="sev {sevClass[f.severity]}"></span>
            <div class="ftext">
              <div><span class="area">{f.area}</span> {stripPobText(f.message)}</div>
              {#if f.fix}<div class="dim small">{f.fix}</div>{/if}
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  <section class="col gear">
    <div class="head">
      <span class="title">{m.opt_gear()}</span>
    </div>
    <div class="controls">
      <div class="ctl">
        <span class="label">{m.opt_aim()}</span>
        <div class="seg" role="radiogroup">
          {#each [["balanced", m.opt_aim_balanced()], ["defence", m.opt_aim_defence()], ["damage", m.opt_aim_damage()]] as [id, label]}
            <button class:on={preset === id} onclick={() => (preset = id as typeof preset)} disabled={running}>{label}</button>
          {/each}
        </div>
      </div>
      <div class="ctl">
        <span class="label">{m.opt_item_level()}</span>
        <input class="input mono ilvl" type="number" min="1" max="100" bind:value={itemLevel} oninput={() => (itemLevelTouched = true)} disabled={running} />
      </div>
      <div class="ctl">
        <span class="label">{m.opt_rolls()}</span>
        <div class="seg" role="radiogroup">
          {#each [[1, m.opt_roll_perfect()], [0.8, m.opt_roll_good()], [0.5, m.opt_roll_average()]] as [v, label]}
            <button class:on={range === v} onclick={() => (range = v as number)} disabled={running}>{label}</button>
          {/each}
        </div>
      </div>
      <div class="ctl slots">
        <span class="label">{m.opt_slots()}</span>
        <div class="chips">
          {#each equipped as e (e.slot)}
            <button class="chip" class:on={chosen.has(e.slot)} disabled={running} title={e.name ?? ""} onclick={() => toggleSlot(e.slot)}>
              {e.slot}{#if e.unique}<span class="dim"> {m.opt_unique()}</span>{/if}
            </button>
          {/each}
          {#if equipped.length === 0}<span class="dim small">{m.opt_no_gear()}</span>{/if}
        </div>
      </div>
      <div class="ctl run">
        {#if running}
          <button class="btn sm" onclick={() => (cancel = true)}>{m.common_stop()}</button>
          <span class="prog">
            <span class="bar"><span class="fill" style:width={progress && progress.total ? `${(100 * progress.done) / progress.total}%` : "0%"}></span></span>
            <span class="dim small">{progress?.note ?? m.opt_starting()}</span>
          </span>
        {:else}
          <button class="btn sm primary" onclick={run} disabled={!build.loaded || chosen.size === 0} title={m.opt_run_title()}>{m.opt_run({ count: chosen.size })}</button>
        {/if}
      </div>
      {#if error}<div class="err small">{error}</div>{/if}
    </div>

    {#if result}
      <div class="results">
        <div class="ghead">
          <span>{m.opt_result()}</span>
          <span class="dim">{m.opt_result_summary({ count: result.proposals.length, seconds: Math.round(result.ms / 100) / 10 })}</span>
          <button class="btn sm primary" onclick={applyAll} disabled={build.busy > 0 || result.proposals.every((p) => applied.has(p.slot))}>{m.opt_apply_all()}</button>
        </div>
        <table class="before">
          <thead><tr><th></th><th class="num">{m.opt_col_now()}</th><th class="num">{m.opt_col_proposed()}</th><th class="num">{m.opt_col_change()}</th></tr></thead>
          <tbody>
            {#each HEADLINE as h}
              {#if result.before[h.key] !== undefined || result.after[h.key] !== undefined}
                <tr>
                  <td>{h.label}</td>
                  <td class="num mono">{fmt(result.before[h.key], h.pct)}</td>
                  <td class="num mono">{fmt(result.after[h.key], h.pct)}</td>
                  <td class="num mono {deltaClass(result.delta[h.key])}">{fmtDelta(result.delta[h.key], h.pct)}</td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
        {#each result.proposals as p (p.slot)}
          <div class="prop" class:done={applied.has(p.slot)}>
            <div class="phead">
              <span class="pslot">{p.slot}</span>
              <span class="pbase">{p.base}</span>
              {#if p.replaces}<span class="dim small">{m.opt_replaces({ item: stripPobText(p.replaces) })}</span>{/if}
              <button class="btn sm" onclick={() => apply(p)} disabled={build.busy > 0 || applied.has(p.slot)}>{applied.has(p.slot) ? m.opt_applied() : m.opt_apply()}</button>
            </div>
            <div class="mods mono">
              {#each p.mods as mod}<div>{mod}</div>{/each}
            </div>
            <div class="lookfor"><span class="dim">{m.opt_look_for()}</span> {p.lookFor.join(", ")}{#if p.implicit}<span class="dim"> {m.opt_implicit()}</span> {plainImplicit(p.implicit)}{/if}</div>
            <div class="deltas">
              {#each DELTA_KEYS as d}
                {#if p.delta[d.key] !== undefined && Math.abs(p.delta[d.key]) >= 0.5}
                  <span class="dchip {deltaClass(p.delta[d.key])}"><span class="mono">{fmtDelta(p.delta[d.key], d.pct)}</span> {d.label}</span>
                {/if}
              {/each}
              {#if p.requirements.str || p.requirements.dex || p.requirements.int}
                <span class="dim small">{m.opt_needs()} <span class="mono">{[p.requirements.str && m.opt_req_str({ value: p.requirements.str }), p.requirements.dex && m.opt_req_dex({ value: p.requirements.dex }), p.requirements.int && m.opt_req_int({ value: p.requirements.int })].filter(Boolean).join(", ")}</span></span>
              {/if}
            </div>
          </div>
        {/each}
        {#each result.skipped as s}
          <div class="dim small pad">{s.slot}: {s.reason}</div>
        {/each}
      </div>
    {/if}
    <div class="head tree">
      <span class="title">{m.opt_tree()}</span>
    </div>
    <div class="controls">
      <div class="ctl">
        <span class="label">{m.opt_tree_stat()}</span>
        <div class="seg" role="radiogroup">
          {#each TREE_STATS as [id, label]}
            <button
              class:on={treeStat === id}
              onclick={() => {
                treeStatChosen = true;
                treeStat = id;
              }}
              disabled={treeRunning || planRunning}>{label}</button
            >
          {/each}
        </div>
      </div>
      <div class="ctl run">
        <button class="btn sm primary" onclick={scanTree} disabled={treeRunning || planRunning || !build.loaded}>{treeRunning ? m.opt_scanning() : m.opt_scan()}</button>
        <label class="chk small"><input type="checkbox" bind:checked={notablesOnly} disabled={treeRunning} /> {m.opt_notables_only()}</label>
        {#if summary}
          <span class="dim small"><span class="mono">{Math.max(0, summary.pointsAvailableMax - summary.passivePointsSpent)}</span> {m.opt_points_unspent()}</span>
        {/if}
      </div>
      <div class="ctl run">
        <span class="label">{m.opt_plan()}</span>
        <input
          class="input sm num budget"
          type="number"
          min="1"
          max="120"
          placeholder={String(unspent || 10)}
          bind:value={planBudget}
          disabled={planRunning}
          title={m.opt_plan_budget_title()}
        />
        <span class="dim small">{m.opt_plan_points()}</span>
        <button class="btn sm" onclick={planTree} disabled={planRunning || treeRunning || !build.loaded}>{planRunning ? m.opt_planning() : m.opt_plan()}</button>
      </div>
      {#if treeError}<div class="err small">{treeError}</div>{/if}
      {#if planError}<div class="err small">{planError}</div>{/if}
    </div>
    {#if plan}
      <div class="results">
        <div class="ghead">
          <span>{m.opt_plan_header()} <span class="mono">{plan.spent}</span> {m.opt_plan_points_label()} <span class="mono up">+{fmtGain(plan.total)}</span> {plan.label}</span>
          <span class="dim">{Math.round(plan.ms / 100) / 10}s</span>
          {#if planStale}<span class="warn">{m.opt_plan_stale()}</span>{/if}
          <button class="btn sm primary" onclick={applyPlan} disabled={build.busy > 0 || planStale || plan.picks.length === 0}>{m.opt_allocate_all()}</button>
        </div>
        {#if plan.picks.length === 0}
          <div class="dim small pad">{m.opt_plan_none({ budget: plan.budget, stat: plan.label })}</div>
        {/if}
        {#each plan.picks as p, i (p.id)}
          <div class="node">
            <span class="nname" title={p.name ?? ""}><span class="dim mono">{i + 1}</span> {p.name}</span>
            <span class="ntype dim small">{p.type}</span>
            <span class="ngain mono up">+{fmtGain(p.gain)}</span>
            <span class="nnote dim small"><span class="mono">{p.cost}</span> {p.cost === 1 ? m.opt_point_one() : m.opt_point_many()}</span>
            <span></span>
          </div>
        {/each}
      </div>
    {/if}
    {#if treeRows}
      <div class="results">
        <div class="ghead"><span>{m.opt_best_to_add({ stat: treeRows.stat })}</span><span class="dim">{Math.round(treeRows.ms / 100) / 10}s</span>{#if treeStale}<span class="warn">{m.opt_scan_stale()}</span>{/if}</div>
        {#each treeRows.best as r (r.id)}
          <div class="node">
            <span class="nname" title={r.name}>{r.name}</span>
            <span class="ntype dim small">{r.type}</span>
            <span class="ngain mono up">+{fmtGain(r.pathPower)}</span>
            <span class="nnote dim small">{m.opt_per_point()} <span class="mono">{r.pathDist}</span> {m.opt_to_reach()}</span>
            <button class="btn sm" onclick={() => allocate(r.id)} disabled={build.busy > 0}>{m.opt_allocate()}</button>
          </div>
        {/each}
        <div class="ghead"><span>{m.opt_weakest()}</span></div>
        {#each treeRows.weakest as r (r.id)}
          <div class="node">
            <span class="nname" title={r.name}>{r.name}</span>
            <span class="ntype dim small">{r.type}</span>
            <span class="ngain mono" class:down={r.power < 0}>{r.power < 0 ? fmtGain(r.power) : "0"}</span>
            <span class="nnote dim small">{m.opt_if_removed()} <span class="mono">{r.pathDist}</span> {m.opt_depend_on_it()}</span>
            <button class="btn sm ghost" onclick={() => remove(r.id)} disabled={build.busy > 0}>{m.common_remove()}</button>
          </div>
        {/each}
      </div>
    {/if}
  </section>
</div>

<style>
  .page {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(340px, 440px) minmax(420px, 1fr);
    grid-template-rows: auto minmax(0, 1fr);
    min-height: 0;
  }
  .disclaimer {
    grid-column: 1 / -1;
    margin: 0;
    padding: 7px 14px;
    background: color-mix(in oklab, var(--bad) 12%, transparent);
    border-bottom: 1px solid color-mix(in oklab, var(--bad) 45%, transparent);
    font-size: var(--fs-xs);
    line-height: 1.45;
    color: var(--fg-1);
  }
  .disclaimer strong {
    color: var(--bad);
    font-weight: 600;
    margin-right: 4px;
  }
  .col {
    display: flex;
    flex-direction: column;
    min-height: 0;
    overflow-y: auto;
  }
  .review {
    border-right: 1px solid var(--line-0);
  }
  .head {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 12px 14px 8px;
    border-bottom: 1px solid var(--line-0);
  }
  .title {
    font-size: var(--fs-md);
    font-weight: 600;
  }
  .pad {
    padding: 8px 14px;
  }
  .facts {
    display: grid;
    grid-template-columns: 1fr;
    padding: 6px 0;
    border-bottom: 1px solid var(--line-0);
  }
  .fact {
    display: grid;
    grid-template-columns: 74px auto;
    align-items: baseline;
    gap: 10px;
    padding: 4px 14px;
    font-size: var(--fs-sm);
  }
  .fact .k {
    color: var(--fg-2);
  }
  .fact .v {
    font-size: var(--fs-md);
  }
  .fact .fire {
    color: var(--c-fire);
  }
  .fact .cold {
    color: var(--c-cold);
  }
  .fact .lightning {
    color: var(--c-lightning);
  }
  .fact .chaos {
    color: var(--c-chaos);
  }
  .fact .bad {
    color: var(--bad);
  }
  .bad {
    color: var(--bad);
  }
  .findings {
    display: flex;
    flex-direction: column;
  }
  .finding {
    display: flex;
    gap: 10px;
    padding: 8px 14px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
    line-height: 1.45;
  }
  .sev {
    flex: 0 0 auto;
    width: 6px;
    height: 6px;
    margin-top: 6px;
    border-radius: 50%;
    background: var(--fg-3);
  }
  .sev.bad {
    background: var(--bad);
  }
  .sev.warn {
    background: var(--warn);
  }
  .area {
    color: var(--fg-2);
    text-transform: capitalize;
  }
  .area::after {
    content: " ·";
  }
  .controls {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 14px;
    border-bottom: 1px solid var(--line-0);
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: var(--fs-sm);
  }
  .ctl .label {
    width: 74px;
    flex: 0 0 auto;
  }
  .ctl.slots {
    align-items: flex-start;
  }
  .seg {
    display: inline-flex;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow: hidden;
  }
  .seg button {
    padding: 3px 10px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    background: transparent;
    border: 0;
    border-right: 1px solid var(--line-1);
  }
  .seg button:last-child {
    border-right: 0;
  }
  .seg button.on {
    color: var(--fg-0);
    background: var(--bg-3);
  }
  .ilvl,
  .budget {
    width: 64px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .chip {
    padding: 2px 8px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    background: transparent;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
  }
  .chip.on {
    color: var(--fg-0);
    border-color: var(--line-2);
    background: var(--bg-3);
  }
  .run {
    margin-top: 2px;
  }
  .prog {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
  }
  .bar {
    width: 160px;
    height: 3px;
    background: var(--bg-3);
    border-radius: 2px;
    overflow: hidden;
  }
  .fill {
    display: block;
    height: 100%;
    background: var(--fg-1);
    transition: width 0.15s;
  }
  .err {
    color: var(--bad);
  }
  .results {
    display: flex;
    flex-direction: column;
  }
  .ghead {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 14px 6px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .ghead .btn {
    margin-left: auto;
  }
  table.before {
    margin: 0 14px 6px;
    border-collapse: collapse;
    font-size: var(--fs-sm);
    width: calc(100% - 28px);
    max-width: 520px;
  }
  table.before th,
  table.before td {
    padding: 3px 8px 3px 0;
    text-align: left;
    border-bottom: 1px solid var(--line-0);
  }
  table.before th {
    font-weight: 500;
    color: var(--fg-3);
  }
  .num {
    text-align: right !important;
  }
  .up {
    color: var(--ok);
  }
  .down {
    color: var(--bad);
  }
  .prop {
    margin: 6px 14px;
    padding: 8px 10px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    font-size: var(--fs-sm);
  }
  .prop.done {
    opacity: 0.6;
  }
  .phead {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .phead .btn {
    margin-left: auto;
  }
  .pslot {
    font-weight: 600;
  }
  .pbase {
    color: var(--fg-2);
  }
  .mods {
    margin: 6px 0;
    font-size: var(--fs-xs);
    color: var(--fg-1);
    line-height: 1.5;
  }
  .lookfor {
    margin: 0 0 6px;
    font-size: var(--fs-xs);
    color: var(--fg-1);
    line-height: 1.5;
  }
  .deltas {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .dchip .mono {
    color: var(--fg-1);
  }
  .dchip.up .mono {
    color: var(--ok);
  }
  .dchip.down .mono {
    color: var(--bad);
  }
  .head.tree {
    border-top: 1px solid var(--line-0);
    margin-top: 8px;
  }
  .node {
    display: grid;
    grid-template-columns: minmax(120px, 1fr) 56px 64px minmax(150px, auto) auto;
    align-items: baseline;
    gap: 10px;
    padding: 5px 14px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
  }
  .nname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .ngain {
    text-align: right;
  }
  .nnote {
    white-space: nowrap;
  }
  .chk {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--fg-2);
  }
  .warn {
    color: var(--warn);
    text-transform: none;
    letter-spacing: 0;
    font-weight: 400;
  }
</style>
