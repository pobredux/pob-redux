<script lang="ts">
  import {
    engine,
    listBuilds,
    fetchBuildCode,
    type BuildEntry,
    type CompareBuild,
    type CompareConfigRow,
    type CompareItemRow,
    type CompareLoadout,
    type CompareSkillRow,
    type CompareSummary,
    type CompareTree,
  } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { stripPobText } from "$lib/pobtext";
  import { slotLabel, statLabel } from "$lib/pob-labels";
  import PobText from "$lib/components/PobText.svelte";
  import BuySimilarDialog from "$lib/components/BuySimilarDialog.svelte";
  import { m } from "$lib/paraglide/messages";

  type Pane = "summary" | "tree" | "items" | "skills" | "config";

  const rarityColor: Record<string, string> = {
    UNIQUE: "var(--c-unique)",
    RARE: "var(--c-rare)",
    MAGIC: "var(--c-magic)",
    NORMAL: "var(--c-normal)",
    RELIC: "var(--c-gem)",
  };

  let entries = $state<CompareBuild[]>([]);
  let active = $state(0);
  let pane = $state<Pane>("summary");
  let onlyDiff = $state(true);
  let busy = $state(false);
  let note = $state("");

  let summary = $state<CompareSummary | null>(null);
  let loadouts = $state<{ mine: CompareLoadout; theirs: CompareLoadout } | null>(null);
  let items = $state<CompareItemRow[]>([]);
  let skills = $state<CompareSkillRow[]>([]);
  let config = $state<CompareConfigRow[]>([]);
  let tree = $state<CompareTree | null>(null);

  // Add-a-build panel
  let picker = $state(false);
  let saved = $state<BuildEntry[]>([]);
  let pickQuery = $state("");
  let codeText = $state("");

  async function guard<T>(fn: () => Promise<T>): Promise<T | null> {
    busy = true;
    try {
      return await fn();
    } catch (e) {
      build.error = String(e).split("\n")[0];
      return null;
    } finally {
      busy = false;
    }
  }

  async function refreshPane() {
    if (!active) {
      summary = null;
      items = [];
      skills = [];
      config = [];
      tree = null;
      loadouts = null;
      return;
    }
    await guard(async () => {
      if (pane === "summary") summary = await engine.compareSummary(onlyDiff);
      else if (pane === "items") items = (await engine.compareItems(onlyDiff)).rows;
      else if (pane === "skills") skills = (await engine.compareSkills(onlyDiff)).rows;
      else if (pane === "config") config = (await engine.compareConfig(onlyDiff)).rows;
      else if (pane === "tree") tree = await engine.compareTree();
      loadouts = await engine.compareLoadouts();
    });
  }

  $effect(() => {
    engine
      .compareList()
      .then((r) => {
        entries = r.entries;
        active = r.active;
      })
      .catch(() => {});
  });

  // Re-pull whichever pane is showing when the build, the pane or the filter changes.
  $effect(() => {
    void pane;
    void onlyDiff;
    void active;
    void build.rev;
    refreshPane();
  });

  async function addFromFile(b: BuildEntry) {
    const r = await guard(() => engine.compareAdd({ path: b.path, label: b.name }));
    if (r) {
      entries = r.entries;
      active = r.active;
      picker = false;
      note = m.compare_added({ name: b.name });
    }
  }

  async function addFromText() {
    const text = codeText.trim();
    if (!text) return;
    const r = await guard(async () => {
      if (/^https?:\/\//i.test(text)) {
        const fetched = await fetchBuildCode(text);
        return engine.compareAdd({ code: fetched.code, label: fetched.site || m.compare_linked_build() });
      }
      if (text.startsWith("<")) return engine.compareAdd({ xml: text, label: m.compare_pasted_build() });
      return engine.compareAdd({ code: text, label: m.compare_pasted_build() });
    });
    if (r) {
      entries = r.entries;
      active = r.active;
      picker = false;
      codeText = "";
      note = m.compare_loaded();
    }
  }

  async function openPicker() {
    picker = !picker;
    if (picker && !saved.length) {
      const list = await listBuilds().catch(() => []);
      saved = list;
    }
  }

  async function pick(index: number) {
    const r = await guard(() => engine.compareSelect(index));
    if (r) {
      entries = r.entries;
      active = r.active;
    }
  }

  async function drop(index: number) {
    const r = await guard(() => engine.compareRemove(index));
    if (r) {
      entries = r.entries;
      active = r.active;
      note = "";
    }
  }

  async function setLoadout(p: { spec?: number; itemSet?: number; skillSet?: number; socketGroup?: number }) {
    const r = await guard(() => engine.compareSetLoadout(p));
    if (r) {
      loadouts = r;
      await refreshPane();
    }
  }

  let buySimilarSlot = $state<string | null>(null);

  async function takeItem(slot: string) {
    const r = await guard(() => engine.compareCopyItem(slot));
    if (r) {
      note = m.compare_item_equipped({ item: r.itemName, slot: r.slot });
      await build.sync();
    }
  }

  async function takeTree() {
    const r = await guard(() => engine.compareCopyTree());
    if (r) {
      note = m.compare_tree_added({ title: r.title, index: r.index });
      await build.sync();
    }
  }

  const shownBuilds = $derived.by(() => {
    const q = pickQuery.trim().toLowerCase();
    return saved.filter((b) => !q || b.name.toLowerCase().includes(q) || (b.folder ?? "").toLowerCase().includes(q));
  });
  const current = $derived(entries.find((e) => e.index === active) ?? null);

  function pct(row: { percent: number | null }) {
    if (row.percent === null || !Number.isFinite(row.percent)) return "";
    const v = row.percent;
    if (Math.abs(v) < 0.05) return "";
    // A tiny baseline turns into a meaningless percentage, so cap it.
    if (v > 999) return "+999%";
    if (v < -999) return "-999%";
    return `${v > 0 ? "+" : ""}${v.toFixed(1)}%`;
  }

  function cfgText(v: string | number | boolean | null) {
    if (v === null || v === undefined) return "—";
    if (v === true) return m.common_on_value();
    if (v === false) return m.common_off_value();
    return String(v);
  }
</script>

<div class="page cmppage">
  <div class="bar">
    <div class="group">
      <span class="label">{m.compare_with()}</span>
      {#if entries.length}
        <select class="select sm cmp" value={active} onchange={(e) => pick(Number((e.target as HTMLSelectElement).value))} disabled={busy}>
          {#each entries as e (e.index)}
            <option value={e.index}>{e.label}{e.className ? ` · ${e.ascendClassName ?? e.className} ${e.level}` : ""}</option>
          {/each}
        </select>
        <button class="btn sm ghost" onclick={() => drop(active)} disabled={busy} title={m.compare_remove_title()}>{m.compare_remove()}</button>
      {:else}
        <span class="dim small">{m.compare_nothing()}</span>
      {/if}
      <button class="btn sm" class:on={picker} onclick={openPicker} disabled={busy}>{m.compare_add()}</button>
    </div>
    {#if entries.length}
      <div class="group">
        <span class="seg">
          {#each [["summary", m.compare_pane_summary()], ["tree", m.compare_pane_tree()], ["items", m.compare_pane_items()], ["skills", m.compare_pane_skills()], ["config", m.compare_pane_config()]] as [id, label] (id)}
            <button class:on={pane === id} onclick={() => (pane = id as Pane)}>{label}</button>
          {/each}
        </span>
        <label class="chk small" title={m.compare_only_diff_title()}>
          <input type="checkbox" bind:checked={onlyDiff} />
          {m.compare_only_diff()}
        </label>
        {#if note}<span class="dim small">{note}</span>{/if}
      </div>
    {/if}
  </div>

  {#if picker}
    <div class="panel picker">
      <div class="prow">
        <input class="input sm grow" placeholder={m.compare_paste_placeholder()} bind:value={codeText} onkeydown={(e) => e.key === "Enter" && addFromText()} />
        <button class="btn sm primary" onclick={addFromText} disabled={busy || !codeText.trim()}>{m.common_load()}</button>
      </div>
      <div class="prow">
        <input class="input sm grow" placeholder={m.compare_search_saved()} bind:value={pickQuery} />
        <span class="dim num small">{shownBuilds.length}</span>
      </div>
      <div class="plist">
        {#each shownBuilds.slice(0, 200) as b (b.path)}
          <button class="brow" onclick={() => addFromFile(b)} disabled={busy}>
            <span class="nm">{b.name}</span>
            {#if b.folder}<span class="dim small">{b.folder}</span>{/if}
            <span class="dim small">{b.ascend_class_name ?? b.class_name ?? ""} {b.level ?? ""}</span>
          </button>
        {:else}
          <div class="dim small pad">{m.compare_no_saved()}</div>
        {/each}
      </div>
    </div>
  {/if}

  {#if !entries.length}
    <div class="empty">
      <p>{m.compare_empty()}</p>
      <p class="dim small">{m.compare_empty_hint()}</p>
    </div>
  {:else}
    {#if loadouts}
      <div class="loadout">
        <span class="olabel">{m.compare_their_loadout()}</span>
        <select class="select sm" value={loadouts.theirs.specs.find((s) => s.active)?.index ?? 1} onchange={(e) => setLoadout({ spec: Number((e.target as HTMLSelectElement).value) })} disabled={busy}>
          {#each loadouts.theirs.specs as s (s.index)}
            <option value={s.index}>{stripPobText(s.title)} · {s.nodes}</option>
          {/each}
        </select>
        <select class="select sm" value={loadouts.theirs.itemSets.find((s) => s.active)?.id ?? 0} onchange={(e) => setLoadout({ itemSet: Number((e.target as HTMLSelectElement).value) })} disabled={busy}>
          {#each loadouts.theirs.itemSets as s (s.id)}
            <option value={s.id}>{stripPobText(s.title)}</option>
          {/each}
        </select>
        <select class="select sm" value={loadouts.theirs.skillSets.find((s) => s.active)?.id ?? 0} onchange={(e) => setLoadout({ skillSet: Number((e.target as HTMLSelectElement).value) })} disabled={busy}>
          {#each loadouts.theirs.skillSets as s (s.id)}
            <option value={s.id}>{stripPobText(s.title)}</option>
          {/each}
        </select>
        <select class="select sm grow" value={loadouts.theirs.socketGroups.find((s) => s.active)?.index ?? 1} onchange={(e) => setLoadout({ socketGroup: Number((e.target as HTMLSelectElement).value) })} disabled={busy}>
          {#each loadouts.theirs.socketGroups as s (s.index)}
            <option value={s.index}>{stripPobText(s.label)}</option>
          {/each}
        </select>
      </div>
    {/if}

    <div class="body">
      {#if pane === "summary"}
        {#if summary}
          <div class="thead">
            <span class="tlabel">{m.compare_col_stat()}</span>
            <span class="tnum">{summary.mine.label}</span>
            <span class="tnum">{current?.label ?? m.compare_fallback_label()}</span>
            <span class="tnum">{m.compare_col_change()}</span>
            <span class="tpct"></span>
          </div>
          <div class="scroll">
            <!-- PoB's stat list repeats a stat under different skill flags, so position is part of the key. -->
            {#each summary.rows as r, i (r.stat + "#" + i)}
              <div class="srow">
                <span class="tlabel">{statLabel(r.label)}</span>
                <span class="tnum num">{r.mineText ?? "—"}</span>
                <span class="tnum num">{r.theirsText ?? "—"}</span>
                <span class="tnum num" class:up={r.better === true} class:down={r.better === false}>{r.deltaText ?? ""}</span>
                <span class="tpct num" class:up={r.better === true} class:down={r.better === false}>{pct(r)}</span>
              </div>
            {:else}
              <div class="dim small pad">{m.compare_same_numbers()}</div>
            {/each}
          </div>
        {/if}
      {:else if pane === "tree"}
        {#if tree}
          <div class="treegrid">
            <div class="tcard">
              <div class="label">{stripPobText(tree.mine.title)}</div>
              <div class="big num">{tree.mine.nodes}</div>
              <div class="dim small">{m.compare_passives_allocated({ className: tree.mine.className })}</div>
              {#if tree.keystonesLost.length}
                <div class="klist">
                  <span class="dim small">{m.compare_only_yours()}</span>
                  {#each tree.keystonesLost as k}<span class="key down">{k}</span>{/each}
                </div>
              {/if}
            </div>
            <div class="tcard">
              <div class="label">{stripPobText(tree.theirs.title)}</div>
              <div class="big num">{tree.theirs.nodes}</div>
              <div class="dim small">{m.compare_passives_allocated({ className: tree.theirs.className })}</div>
              {#if tree.keystonesGained.length}
                <div class="klist">
                  <span class="dim small">{m.compare_only_theirs()}</span>
                  {#each tree.keystonesGained as k}<span class="key up">{k}</span>{/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="tsum">
            <span><b class="num up">{tree.gained.length}</b> {m.compare_gained()}</span>
            <span><b class="num down">{tree.lost.length}</b> {m.compare_lost()}</span>
            <button class="btn sm" onclick={takeTree} disabled={busy}>{m.compare_copy_tree()}</button>
          </div>
          <p class="dim small hint">{m.compare_copy_tree_hint()}</p>
        {/if}
      {:else if pane === "items"}
        <div class="thead">
          <span class="tlabel">{m.compare_col_slot()}</span>
          <span class="tside">{m.compare_col_yours()}</span>
          <span class="tside">{m.compare_col_theirs()}</span>
          <span class="tact"></span>
        </div>
        <div class="scroll">
          {#each items as r (r.slot)}
            <div class="srow">
              <span class="tlabel">{slotLabel(r.label ?? r.slot)}</span>
              <span class="tside" style:color={rarityColor[r.mine?.rarity ?? ""] ?? "var(--fg-2)"}>{r.mine?.name ?? "—"}</span>
              <span class="tside" style:color={rarityColor[r.theirs?.rarity ?? ""] ?? "var(--fg-2)"}>{r.theirs?.name ?? "—"}</span>
              <span class="tact">
                {#if r.theirs}
                  <button class="btn sm ghost" onclick={() => (buySimilarSlot = r.slot)} disabled={busy} title={m.compare_buy_similar_title()}>{m.compare_buy_similar()}</button>
                  <button class="btn sm ghost" onclick={() => takeItem(r.slot)} disabled={busy} title={m.compare_use_theirs_title()}>{m.compare_use_theirs()}</button>
                {/if}
              </span>
            </div>
          {:else}
            <div class="dim small pad">{m.compare_same_items()}</div>
          {/each}
        </div>
      {:else if pane === "skills"}
        <div class="thead">
          <span class="tlabel">{m.compare_col_group()}</span>
          <span class="tside">{m.compare_col_yours()}</span>
          <span class="tside">{m.compare_col_theirs()}</span>
        </div>
        <div class="scroll">
          {#each skills as r (r.index)}
            <div class="srow tall">
              <span class="tlabel"><PobText text={r.mine?.label ?? r.theirs?.label ?? m.compare_group_fallback({ index: r.index })} /></span>
              <span class="tside gems">
                {#each r.mine?.gems ?? [] as g}
                  <span class="gem" class:off={!g.enabled}>{g.name} <span class="dim num">{g.level}/{g.quality}</span></span>
                {:else}
                  <span class="dim small">—</span>
                {/each}
              </span>
              <span class="tside gems">
                {#each r.theirs?.gems ?? [] as g}
                  <span class="gem" class:off={!g.enabled}>{g.name} <span class="dim num">{g.level}/{g.quality}</span></span>
                {:else}
                  <span class="dim small">—</span>
                {/each}
              </span>
            </div>
          {:else}
            <div class="dim small pad">{m.compare_same_skills()}</div>
          {/each}
        </div>
      {:else}
        <div class="thead">
          <span class="tlabel">{m.compare_col_option()}</span>
          <span class="tside">{m.compare_col_yours()}</span>
          <span class="tside">{m.compare_col_theirs()}</span>
        </div>
        <div class="scroll">
          {#each config as r (r.var)}
            <div class="srow">
              <span class="tlabel" title={r.var}>{r.label}</span>
              <span class="tside num">{cfgText(r.mine)}</span>
              <span class="tside num">{cfgText(r.theirs)}</span>
            </div>
          {:else}
            <div class="dim small pad">{m.compare_same_config()}</div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
  {#if buySimilarSlot}
    <BuySimilarDialog target={{ slot: buySimilarSlot, side: "theirs" }} onclose={() => (buySimilarSlot = null)} />
  {/if}
</div>

<style>
  .page {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .bar {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px 16px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line-0);
  }
  .group {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .cmp {
    min-width: 200px;
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
    cursor: var(--cursor-pointer, pointer);
  }
  .seg button.on {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .picker {
    margin: 8px 10px 0;
    padding: 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .prow {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .prow .grow {
    flex: 1;
    min-width: 0;
  }
  .plist {
    max-height: 32vh;
    overflow: auto;
    border: 1px solid var(--line-0);
    border-radius: var(--r-2);
  }
  .brow {
    display: flex;
    align-items: baseline;
    gap: 10px;
    width: 100%;
    padding: 4px 8px;
    background: none;
    border: 0;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: var(--cursor-pointer, pointer);
  }
  .brow:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .brow .nm {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 28px 24px;
    max-width: 560px;
  }
  .empty p {
    margin: 0 0 8px;
  }
  .loadout {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 10px;
    border-bottom: 1px solid var(--line-0);
  }
  .loadout .grow {
    flex: 1;
    min-width: 0;
  }
  .olabel {
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    padding: 0 10px 10px;
  }
  .thead,
  .srow {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 3px 6px;
  }
  .thead {
    font-size: var(--fs-xs);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--fg-2);
    border-bottom: 1px solid var(--line-0);
  }
  .srow:hover {
    background: var(--bg-hover);
  }
  .srow.tall {
    align-items: flex-start;
  }
  .tlabel {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tnum {
    width: 130px;
    flex: none;
    text-align: right;
  }
  .tpct {
    width: 64px;
    flex: none;
    text-align: right;
  }
  .tside {
    width: 30%;
    flex: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tact {
    width: 190px;
    flex: none;
    text-align: right;
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  .up {
    color: var(--ok);
  }
  .down {
    color: var(--bad);
  }
  .pad {
    padding: 10px 6px;
  }
  .treegrid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
    padding-top: 10px;
  }
  .tcard {
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    padding: 10px 12px;
  }
  .big {
    font-size: 28px;
    line-height: 1.2;
  }
  .klist {
    display: flex;
    flex-wrap: wrap;
    align-items: baseline;
    gap: 6px;
    margin-top: 8px;
  }
  .key {
    font-size: var(--fs-xs);
    padding: 1px 6px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
  }
  .tsum {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 12px 2px 0;
  }
  .hint {
    margin: 6px 2px 0;
  }
  .gems {
    display: flex;
    flex-wrap: wrap;
    gap: 3px 8px;
    white-space: normal;
  }
  .gem {
    font-size: var(--fs-xs);
  }
  .gem.off {
    color: var(--fg-3);
    text-decoration: line-through;
  }
</style>
