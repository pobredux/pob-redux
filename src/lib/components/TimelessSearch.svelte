<script lang="ts">
  import { tradeWindow } from "$lib/state/trade.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import {
    engine,
    type TimelessInfo,
    type TimelessNode,
    type TimelessResult,
    type TimelessWant,
  } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { m } from "$lib/paraglide/messages";
  import SearchSelect from "./SearchSelect.svelte";
  import SocketPreview from "./SocketPreview.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const LEAGUE_KEY = "pob-redux:trade-league";

  function savedLeague() {
    try {
      return localStorage.getItem(LEAGUE_KEY) ?? "";
    } catch {
      return "";
    }
  }

  let info = $state<TimelessInfo | null>(null);
  let jewelType = $state(2);
  let conqueror = $state(1);
  let socket = $state(0);
  let allocatedOnly = $state(false);
  let reach = $state(0);
  let league = $state(savedLeague());
  let leagues = $state<{ id: string; text: string }[] | null>(null);
  let search = $state("");
  let wanted = $state<(TimelessWant & { name: string })[]>([]);
  let keep = $state<string[]>([]);

  let running = $state(false);
  let progress = $state(0);
  let checked = $state(0);
  let total = $state(0);
  let result = $state<TimelessResult | null>(null);
  let selected = $state<number[]>([]);
  let note = $state("");
  let cancel = false;

  const jewel = $derived(info?.jewels.find((j) => j.id === jewelType) ?? null);
  const socketNode = $derived(info?.sockets.find((s) => s.id === socket) ?? null);
  const socketOptions = $derived((info?.sockets ?? []).map((s) => ({ value: String(s.id), label: `${s.allocated ? "● " : ""}${s.label}` })));

  $effect(() => {
    const t = jewelType;
    const s = socket;
    engine
      .timelessInfo(t, s || undefined)
      .then((r) => {
        info = r;
        if (!s && r.sockets.length) socket = (r.sockets.find((x) => x.allocated) ?? r.sockets[0]).id;
      })
      .catch((e) => (build.error = String(e)));
  });

  $effect(() => {
    engine
      .tradeLeagues()
      .then((r) => {
        leagues = r.leagues;
        if (!r.leagues.some((l) => l.id === league) && r.leagues.length) league = r.leagues[0].id;
      })
      .catch(() => {
        leagues = null;
        if (!league) league = "Standard";
      });
  });

  // The wanted list is written against one jewel's node ids, so it cannot
  // carry over to another jewel.
  let lastJewel = -1;
  $effect(() => {
    if (jewelType !== lastJewel) {
      lastJewel = jewelType;
      wanted = [];
      keep = [];
      result = null;
      selected = [];
      note = "";
    }
  });

  const wantedIds = $derived(new Set(wanted.map((w) => w.id)));
  const options = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return (info?.nodes ?? []).filter(
      (n) =>
        !wantedIds.has(n.id) &&
        (!q || n.name.toLowerCase().includes(q) || n.stats.some((s) => s.toLowerCase().includes(q))),
    );
  });
  const radiusNotables = $derived((info?.radius ?? []).filter((n) => n.notable || n.keystone));

  function add(n: TimelessNode) {
    wanted = [...wanted, { id: n.id, name: n.name, weight: 1 }];
  }
  function drop(id: string) {
    wanted = wanted.filter((w) => w.id !== id);
  }
  function setWeight(id: string, field: "weight" | "weight2" | "minWeight", value: string) {
    const n = value.trim() === "" ? undefined : Number(value);
    wanted = wanted.map((w) => (w.id === id ? { ...w, [field]: Number.isFinite(n as number) ? n : undefined } : w));
  }

  async function run() {
    if (!wanted.length || !socket) return;
    running = true;
    cancel = false;
    result = null;
    selected = [];
    note = "";
    progress = 0;
    checked = 0;
    const started = performance.now();
    try {
      let st = await engine.timelessSearchStart({
        jewelType,
        socket,
        desired: wanted.map((w) => ({ id: w.id, weight: w.weight, weight2: w.weight2, minWeight: w.minWeight })),
        protect: jewelType === 4 ? keep : undefined,
        socketFilter: allocatedOnly,
        socketFilterDistance: allocatedOnly ? reach : 0,
      });
      total = st.total;
      while (!st.done && !cancel) {
        st = await engine.timelessSearchStep(120);
        progress = st.progress;
        checked = st.checked;
        total = st.total;
      }
      result = await engine.timelessSearchResult(200);
      const ms = performance.now() - started;
      const took = ms < 1000 ? `${Math.round(ms)}ms` : `${(ms / 1000).toFixed(1)}s`;
      note = m.timeless_summary({ found: result.found.toLocaleString(), total: result.total.toLocaleString(), took });
    } catch (e) {
      build.error = String(e);
    } finally {
      running = false;
    }
  }

  function toggle(seed: number, e: MouseEvent) {
    if (e.shiftKey && selected.length) {
      const seeds = (result?.results ?? []).map((r) => r.seed);
      const from = seeds.indexOf(selected[selected.length - 1]);
      const to = seeds.indexOf(seed);
      if (from >= 0 && to >= 0) {
        const [a, b] = from < to ? [from, to] : [to, from];
        selected = Array.from(new Set([...selected, ...seeds.slice(a, b + 1)]));
        return;
      }
    }
    selected = selected.includes(seed) ? selected.filter((s) => s !== seed) : [...selected, seed];
  }

  const tradeSeeds = $derived(
    selected.length ? selected : (result?.results ?? []).slice(0, 10).map((r) => r.seed),
  );

  async function trade() {
    if (!tradeSeeds.length) return;
    try {
      localStorage.setItem(LEAGUE_KEY, league);
    } catch {}
    try {
      const r = await engine.timelessTradeUrl({ jewelType, seeds: tradeSeeds, conqueror, league });
      await tradeWindow.open(r.url);
      note = m.timeless_trade_opened({ count: r.seeds });
    } catch (e) {
      build.error = String(e);
    }
  }

  async function copySeeds() {
    if (!tradeSeeds.length) return;
    try {
      await writeText(tradeSeeds.join("\n"));
      note = m.timeless_seeds_copied({ count: tradeSeeds.length });
    } catch (e) {
      build.error = String(e);
    }
  }
</script>

<div class="modal">
  <div class="panel dialog tldlg">
    <div class="head">
      <span class="label">{m.timeless_title()}</span>
      {#if socketNode}
        <span class="dim small">{m.timeless_in_range({ count: info?.radius.length ?? 0, socket: socketNode.label })}</span>
      {/if}
    </div>

    <div class="filters">
      <select class="select sm" bind:value={jewelType} disabled={running} title={m.timeless_jewel_title()}>
        {#each info?.jewels ?? [] as j}
          <option value={j.id}>{j.label}</option>
        {/each}
      </select>
      <select class="select sm" bind:value={conqueror} disabled={running} title={m.timeless_conqueror_title()}>
        {#each jewel?.conquerors ?? [] as c}
          <option value={c.id}>{c.label}</option>
        {/each}
      </select>
      <div class="grow">
        <SearchSelect value={String(socket)} options={socketOptions} label={m.timeless_socket_title()} disabled={running} onchange={(v) => (socket = Number(v))}>
          {#snippet aside(v)}
            <SocketPreview socket={Number(v)} radius={info?.radiusSize ?? 1800} label={info?.sockets.find((s) => String(s.id) === v)?.label ?? ""} />
          {/snippet}
        </SearchSelect>
      </div>
      <label class="chk small" title={m.timeless_taken_only_title()}>
        <input type="checkbox" bind:checked={allocatedOnly} disabled={running} />
        {m.timeless_taken_only()}
      </label>
      {#if allocatedOnly}
        <input class="input num sm" type="number" min="0" max="20" bind:value={reach} disabled={running} title={m.timeless_reach_title()} />
      {/if}
    </div>

    <div class="cols">
      <section class="side">
        <div class="shead">
          {m.timeless_wanted_source()}
          <span class="dim num">{options.length}</span>
        </div>
        <input class="input sm srch" placeholder={m.timeless_search_placeholder()} bind:value={search} />
        <div class="scroll">
          {#each options as n (n.id)}
            <button class="row" onclick={() => add(n)} title={n.stats.join("\n")}>
              <span class="nm">{n.name}</span>
              {#if n.total}<span class="tag">{m.timeless_pooled()}</span>{/if}
            </button>
          {:else}
            <div class="dim small pad">{m.common_nothing_matches()}</div>
          {/each}
        </div>
      </section>

      <section class="side">
        <div class="shead">
          {m.timeless_wanted()}
          <span class="dim num">{wanted.length}</span>
          {#if wanted.length}
            <span class="dim small cols3" class:three={jewelType === 1}>
              <span>{m.timeless_col_weight()}</span>{#if jewelType === 1}<span>{m.timeless_col_second()}</span>{/if}<span>{m.timeless_col_minimum()}</span>
            </span>
          {/if}
        </div>
        <div class="scroll">
          {#each wanted as w (w.id)}
            <div class="want">
              <span class="nm" title={w.id}>{w.name}</span>
              <input
                class="input num xs"
                type="number"
                step="0.1"
                value={w.weight ?? 1}
                oninput={(e) => setWeight(w.id, "weight", (e.target as HTMLInputElement).value)}
                title={m.timeless_weight_title()}
              />
              {#if jewelType === 1}
                <input
                  class="input num xs"
                  type="number"
                  step="0.1"
                  value={w.weight2 ?? ""}
                  placeholder={m.timeless_col_second()}
                  oninput={(e) => setWeight(w.id, "weight2", (e.target as HTMLInputElement).value)}
                  title={m.timeless_weight2_title()}
                />
              {/if}
              <input
                class="input num xs"
                type="number"
                step="0.1"
                value={w.minWeight ?? ""}
                placeholder={m.timeless_min_placeholder()}
                oninput={(e) => setWeight(w.id, "minWeight", (e.target as HTMLInputElement).value)}
                title={m.timeless_min_title()}
              />
              <button class="x" onclick={() => drop(w.id)} title={m.common_remove()}>✕</button>
            </div>
          {:else}
            <div class="dim small pad">{m.timeless_wanted_empty()}</div>
          {/each}
        </div>
        {#if jewelType === 4 && radiusNotables.length}
          <div class="shead">{m.timeless_keep()}</div>
          <div class="scroll short">
            {#each radiusNotables as n (n.id)}
              <label class="chk small keeprow">
                <input
                  type="checkbox"
                  checked={keep.includes(n.name)}
                  onchange={(e) =>
                    (keep = (e.target as HTMLInputElement).checked
                      ? [...keep, n.name]
                      : keep.filter((k) => k !== n.name))}
                />
                {n.name}
                {#if n.keystone}<span class="tag">{m.timeless_keystone()}</span>{/if}
              </label>
            {/each}
          </div>
        {/if}
      </section>
    </div>

    <div class="runbar">
      <button class="btn primary" onclick={run} disabled={running || !wanted.length || !socket}>
        {running ? m.timeless_searching() : m.timeless_search()}
      </button>
      {#if running}
        <button class="btn ghost sm" onclick={() => (cancel = true)}>{m.common_stop()}</button>
        <div class="prog"><span style:width={`${Math.round(progress * 100)}%`}></span></div>
        <span class="dim num small">{checked.toLocaleString()} / {total.toLocaleString()}</span>
      {:else if note}
        <span class="dim small">{note}</span>
      {:else if jewel}
        <span class="dim num small">{m.timeless_seed_range({ min: jewel.seedMin.toLocaleString(), max: jewel.seedMax.toLocaleString() })}</span>
      {/if}
    </div>

    {#if result}
      <div class="side results">
        <div class="shead">
          {m.timeless_best_seeds()}
          <span class="dim num">{result.results.length}</span>
          <span class="dim small">{m.timeless_pick_hint()}</span>
        </div>
        <div class="scroll">
          {#each result.results as r (r.seed)}
            <button class="res" class:on={selected.includes(r.seed)} onclick={(e) => toggle(r.seed, e)}>
              <span class="seed num">{r.seed.toLocaleString()}</span>
              <span class="score num">{r.weight.toFixed(1)}</span>
              <span class="hits">
                {#each r.nodes as n}
                  <span class="hit" title={n.targets.join(", ")}>{n.name} <span class="dim num">×{n.targets.length}</span></span>
                {/each}
              </span>
            </button>
          {:else}
            <div class="dim small pad">{m.timeless_no_results()}</div>
          {/each}
        </div>
      </div>
    {/if}

    <div class="acts">
      {#if result && result.results.length}
        {#if leagues}
          <select class="select sm lg" bind:value={league} title={m.timeless_league_title()}>
            {#each leagues as l (l.id)}
              <option value={l.id}>{l.text}</option>
            {/each}
          </select>
        {:else}
          <input class="input sm lg" bind:value={league} title={m.timeless_league_title()} />
        {/if}
        <button class="btn sm ghost" onclick={copySeeds}>{m.timeless_copy_seeds()}</button>
        <button class="btn sm" onclick={trade} title={m.timeless_trade_title()}>
          {m.timeless_trade({ count: tradeSeeds.length })}
        </button>
        <span class="sp"></span>
      {/if}
      <button class="btn ghost" onclick={onclose}>{m.common_close()}</button>
    </div>
  </div>
</div>

<style>
  .modal {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--backdrop);
    z-index: 6;
  }
  .tldlg {
    width: 900px;
    max-height: 88vh;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 10px;
  }
  .filters {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .filters .grow {
    flex: 1;
    min-width: 0;
  }
  .cols {
    display: grid;
    grid-template-columns: 1fr 1.25fr;
    gap: 10px;
    min-height: 0;
  }
  .side {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
  }
  .shead {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 6px 10px;
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg-2);
    border-bottom: 1px solid var(--line-0);
  }
  .shead .small {
    margin-left: auto;
    text-transform: none;
    letter-spacing: 0;
  }
  .cols3 {
    display: grid;
    grid-template-columns: repeat(2, 60px);
    padding-right: 16px;
    text-align: right;
  }
  .cols3.three {
    grid-template-columns: repeat(3, 60px);
  }
  .srch {
    margin: 6px 6px 0;
  }
  .scroll {
    overflow: auto;
    height: 30vh;
  }
  .scroll.short {
    height: 12vh;
  }
  .row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    width: 100%;
    padding: 4px 10px;
    background: none;
    border: 0;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .row:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .nm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tag {
    color: var(--c-spirit);
    font-size: var(--fs-xs);
  }
  .want {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
  }
  .want:hover {
    background: var(--bg-hover);
  }
  .want .x {
    background: none;
    border: 0;
    color: var(--fg-3);
    cursor: pointer;
    font: inherit;
  }
  .want .x:hover {
    color: var(--bad);
  }
  .keeprow {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 2px 10px;
  }
  .num.xs {
    width: 54px;
    text-align: right;
  }
  .num.sm {
    width: 64px;
    text-align: right;
  }
  .pad {
    padding: 8px 10px;
  }
  .runbar {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .prog {
    flex: 1;
    height: 4px;
    background: var(--bg-2);
    border-radius: 2px;
    overflow: hidden;
  }
  .prog span {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 80ms linear;
  }
  .results .scroll {
    height: 26vh;
  }
  .res {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    padding: 3px 10px;
    background: none;
    border: 0;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .res:hover {
    background: var(--bg-hover);
  }
  .res.on {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .res .seed {
    width: 72px;
    text-align: right;
  }
  .res .score {
    width: 56px;
    text-align: right;
    color: var(--fg-2);
  }
  .hits {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    gap: 4px 10px;
    font-size: var(--fs-xs);
  }
  .acts {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 8px;
  }
  .acts .lg {
    width: 150px;
  }
  .acts .sp {
    flex: 1;
  }
</style>
