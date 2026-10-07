<script lang="ts">
  import { onDestroy } from "svelte";
  import { engine, type ItemCustomization, type ItemCustomizationEdit, type ItemTarget, type ItemSocket, type AnointInfo } from "$lib/engine.svelte";
  import { m } from "$lib/paraglide/messages";
  import EnchantDialog from "./EnchantDialog.svelte";
  import SearchSelect from "./SearchSelect.svelte";

  let { data, target, busy, onchange }: {
    data: ItemCustomization;
    target: ItemTarget;
    busy: boolean;
    onchange: (edit: ItemCustomizationEdit) => unknown;
  } = $props();

  const shape = $derived(data.shape);
  const crucible = $derived(data.crucible.available ? data.crucible : null);
  let crucibleOpen = $state(false);
  const crucibleShown = $derived(!!crucible && (crucibleOpen || crucible.selected.some(Boolean)));
  const anointFlags = $derived(data.anoints);
  const corruptInfo = $derived(data.corruptions);
  const enchantable = $derived(data.enchantable);
  let enchantOpen = $state(false);
  let error = $state<string | null>(null);
  let alive = true;
  onDestroy(() => { alive = false; });
  function setCrucible(node: number, id: string) {
    if (busy || !crucible) return;
    onchange({ operation: "crucible", selected: crucible.selected.map((s, i) => (i === node ? id : s)) });
  }
  const SOCKET_NAMES = $derived<Record<string, string>>({ R: m.items_socket_red(), G: m.items_socket_green(), B: m.items_socket_blue(), W: m.items_socket_white(), A: m.items_socket_abyssal() });

  // PoB allows at most two influences.
  function toggleInfluence(key: string) {
    if (busy || !shape) return;
    const on = shape.influences.filter((i) => i.on).map((i) => i.key);
    const next = on.includes(key) ? on.filter((k) => k !== key) : [...on, key].slice(-2);
    onchange({ operation: "shape", influences: next });
  }

  function setSocket(index: number, patch: Partial<ItemSocket>) {
    if (busy || !shape) return;
    const next = shape.sockets.map((s, i) => (i === index ? { ...s, ...patch } : s));
    onchange({ operation: "shape", sockets: next });
  }

  function addSocket() {
    if (busy || !shape) return;
    const next = [...shape.sockets, { colour: "W", group: shape.sockets.length ? shape.sockets[shape.sockets.length - 1].group : 0 }];
    onchange({ operation: "shape", sockets: next });
  }

  function removeSocket(index: number) {
    if (busy || !shape) return;
    const next = shape.sockets.filter((_, i) => i !== index);
    onchange({ operation: "shape", sockets: next });
  }

  function toggleLink(index: number) {
    if (!shape || index === 0) return;
    const prev = shape.sockets[index - 1];
    const cur = shape.sockets[index];
    const linked = prev.group === cur.group;
    const next = shape.sockets.map((s, i) => (i >= index ? { ...s, group: linked ? s.group + 1 : prev.group } : s));
    if (!busy) onchange({ operation: "shape", sockets: next });
  }

  let anointOpen = $state(false);
  let anointInfo = $state<AnointInfo | null>(null);
  let anointQuery = $state("");
  let anointSlot = $state(1);
  async function openAnoint() {
    if (busy) return;
    const current = target;
    const result = await engine.itemAnoints(current, true).catch((e) => {
      error = String(e);
      return null;
    });
    if (!alive || target !== current) return;
    anointInfo = result;
    anointQuery = "";
    anointSlot = 1;
    anointOpen = anointInfo != null;
  }
  const anointList = $derived.by(() => {
    if (!anointInfo) return [];
    const words = anointQuery.trim().toLowerCase().split(/\s+/).filter(Boolean);
    const list = words.length
      ? anointInfo.nodes.filter((n) => {
          const hay = `${n.name} ${n.stats.join(" ")} ${n.recipe.join(" ")}`.toLowerCase();
          return words.every((w) => hay.includes(w));
        })
      : anointInfo.nodes;
    return list.slice(0, 120);
  });
  async function applyAnoint(nodeId: number | null) {
    if (busy) return;
    anointOpen = false;
    await onchange({ operation: "anoint", nodeId, slot: anointSlot });
  }

  let corruptOpen = $state(false);
  let corruptSel = $state<string[]>([]);
  let corruptRanges = $state<Record<number, number>>({});
  function openCorrupt() {
    if (!corruptInfo) return;
    corruptSel = Array(corruptInfo.enchantNum).fill("");
    const r: Record<number, number> = {};
    for (const rng of corruptInfo.ranges) r[rng.index] = rng.current;
    corruptRanges = r;
    corruptOpen = true;
  }
  const corruptModList = $derived(corruptInfo ? [...corruptInfo.mods, ...corruptInfo.specialMods] : []);
  function corruptOptions(i: number) {
    const takenGroups = corruptSel.filter((id, j) => j !== i && id).map((id) => corruptModList.find((o) => o.id === id)?.group);
    return corruptModList.filter((o) => !o.group || !takenGroups.includes(o.group));
  }
  async function applyCorrupt(mode: "implicits" | "ranges") {
    if (busy) return;
    corruptOpen = false;
    const p: Extract<ItemCustomizationEdit, { operation: "corruption" }> = { operation: "corruption" };
    if (mode === "implicits") p.modIds = corruptSel.filter(Boolean);
    else p.ranges = Object.entries(corruptRanges).map(([index, value]) => ({ index: Number(index), value }));
    await onchange(p);
  }
</script>

<fieldset disabled={busy}>
  <div class="modrow">
    {#if anointFlags?.anointable}
      <button class="btn sm" onclick={openAnoint}>
        {anointFlags.current.length ? `Anoint: ${anointFlags.current.join(", ")}` : "Anoint…"}
      </button>
    {/if}
    {#if enchantable}
      <button class="btn sm" onclick={() => (enchantOpen = true)}>{m.items_enchant()}</button>
    {/if}
    {#if corruptInfo?.corruptible || corruptInfo?.corrupted}
      <button class="btn sm" onclick={openCorrupt}>{corruptInfo.corrupted ? m.items_corrupted_modify() : m.items_corrupt()}</button>
    {/if}
    {#if crucible && !crucibleShown}
      <button class="btn sm" onclick={() => (crucibleOpen = true)}>{m.items_crucible_open()}</button>
    {/if}
  </div>
  {#if shape && (shape.canBeInfluenced || shape.socketLimit > 0 || shape.cluster || crucibleShown)}
    <div class="shape">
      {#if shape.canBeInfluenced}
        <div class="srow">
          <span class="label">{m.items_influence()}</span>
          <span class="chips">
            {#each shape.influences as inf (inf.key)}
              <button class="chip" class:on={inf.on} title={m.items_influence_title()} onclick={() => toggleInfluence(inf.key)}>{inf.name}</button>
            {/each}
          </span>
        </div>
      {/if}
      {#if shape.socketLimit > 0}
        <div class="srow">
          <span class="label">{m.items_sockets()}</span>
          <span class="socks">
            {#each shape.sockets as sock, i (i)}
              {#if i > 0}
                <button
                  class="link"
                  class:on={shape.sockets[i - 1].group === sock.group}
                  title={shape.sockets[i - 1].group === sock.group ? m.items_linked_title() : m.items_not_linked_title()}
                  onclick={() => toggleLink(i)}>—</button
                >
              {/if}
              <span class="sock">
                <select class="select xs sockc" value={sock.colour} onchange={(e) => setSocket(i, { colour: (e.target as HTMLSelectElement).value })}>
                  {#each shape.colours as c}
                    <option value={c}>{SOCKET_NAMES[c] ?? c}</option>
                  {/each}
                </select>
                <button class="mini x" title={m.items_remove_socket()} onclick={() => removeSocket(i)}>✕</button>
              </span>
            {/each}
            {#if shape.sockets.length < shape.socketLimit}
              <button class="btn sm ghost" onclick={addSocket}>{m.items_add_socket()}</button>
            {/if}
          </span>
        </div>
      {/if}
      {#if crucible && crucibleShown}
        <div class="srow cruc">
          <span class="label">{m.items_crucible()}</span>
          <div class="crucnodes">
            {#each crucible.nodes as options, i (i)}
              <div class="crucsel">
                <SearchSelect
                  value={crucible.selected[i] ?? ""}
                  options={[{ value: "", label: m.items_crucible_node_empty({ index: i + 1 }) }, ...options.map((o) => ({ value: o.id, label: `T${o.tier} · ${o.label}` }))]}
                  label={m.items_crucible_node({ index: i + 1 })}
                  onchange={(id) => setCrucible(i, id)}
                />
              </div>
            {/each}
          </div>
        </div>
      {/if}
      {#if shape.cluster}
        <div class="srow">
          <span class="label">{m.items_cluster()}</span>
          <div class="clusterpick">
            <SearchSelect
              value={shape.cluster.skill ?? ""}
              options={[{ value: "", label: m.items_cluster_default() }, ...shape.cluster.skills.map((sk) => ({ value: sk.id, label: sk.name }))]}
              label={m.items_cluster()}
              onchange={(skill) => onchange({ operation: "shape", clusterSkill: skill })}
            />
          </div>
          <label class="fld-inline" title={m.items_cluster_passives_title()}>
            <span class="label">{m.items_cluster_passives()}</span>
            <input
              class="input xs num"
              type="number"
              min={shape.cluster.minNodes}
              max={shape.cluster.maxNodes}
              value={shape.cluster.nodeCount}
              onchange={(e) => onchange({ operation: "shape", clusterNodeCount: Number((e.target as HTMLInputElement).value) })}
            />
          </label>
        </div>
      {/if}
    </div>
  {/if}
  {#if anointOpen && anointInfo}
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1">
      <div class="panel dialog anointdlg">
        <div class="label">{m.items_anoint_title()}</div>
        <div class="crow">
          <input class="input grow2" placeholder={m.items_anoint_search()} bind:value={anointQuery} />
          {#if anointInfo.slots > 1}
            <select class="select" bind:value={anointSlot} title={m.items_anoint_slot()}>
              {#each Array(anointInfo.slots) as _, i}
                <option value={i + 1}>{m.items_anoint_slot_n({ index: i + 1 })}</option>
              {/each}
            </select>
          {/if}
        </div>
        <div class="anointlist">
          {#each anointList as n (n.id)}
            <button class="arow" class:alloc={n.allocated} title={n.stats.join("\n")} onclick={() => applyAnoint(n.id)}>
              <span class="aname">{n.name}</span>
              <span class="dim small">{n.recipe.join(" + ")}</span>
            </button>
          {/each}
          {#if !anointList.length}
            <div class="dim small pad">{m.items_anoint_none()}</div>
          {/if}
        </div>
        <div class="actions">
          {#if anointInfo.current.length}
            <button class="btn" onclick={() => applyAnoint(null)}>{m.items_anoint_remove({ name: anointInfo.current[anointSlot - 1] ?? m.items_anoint_fallback() })}</button>
          {/if}
          <button class="btn ghost" onclick={() => (anointOpen = false)}>{m.common_cancel()}</button>
        </div>
      </div>
    </div>
  {/if}

  {#if corruptOpen && corruptInfo}
    <div class="modal" role="dialog" aria-modal="true" tabindex="-1">
      <div class="panel dialog">
        <div class="label">{m.items_corrupt_title()}</div>
        {#each corruptSel as sel, i}
          <div class="crow">
            <span class="clabel">{m.items_implicit_n({ index: i + 1 })}</span>
            <div class="grow2">
              <SearchSelect
                value={sel}
                options={[{ value: "", label: m.items_implicit_none() }, ...corruptOptions(i).map((opt) => ({ value: opt.id, label: opt.label }))]}
                label={m.items_implicit_n({ index: i + 1 })}
                onchange={(id) => (corruptSel[i] = id)}
              />
            </div>
          </div>
        {/each}
        {#if corruptInfo.ranges.length}
          <div class="label">{m.items_roll_ranges()}</div>
          {#each corruptInfo.ranges as r (r.index)}
            <div class="crow">
              <input
                class="range grow2"
                type="range"
                min="0.78"
                max="1.22"
                step="0.01"
                value={corruptRanges[r.index] ?? 1}
                onchange={(e) => (corruptRanges[r.index] = Number((e.target as HTMLInputElement).value))}
              />
              <span class="num small">{(corruptRanges[r.index] ?? 1).toFixed(2)}</span>
              <span class="dim small rline">{r.line}</span>
            </div>
          {/each}
        {/if}
        <div class="actions">
          <button class="btn primary" onclick={() => applyCorrupt("implicits")} disabled={busy}>{m.items_corrupt_implicits()}</button>
          {#if corruptInfo.ranges.length}
            <button class="btn" onclick={() => applyCorrupt("ranges")} disabled={busy}>{m.items_corrupt_ranges()}</button>
          {/if}
          <button class="btn ghost" onclick={() => (corruptOpen = false)}>{m.common_cancel()}</button>
        </div>
      </div>
    </div>
  {/if}
</fieldset>
{#if error}<p class="err" role="alert">{error}</p>{/if}
{#if enchantOpen}
  <EnchantDialog {target} revision={data.raw} {busy} {onchange} onclose={() => (enchantOpen = false)} />
{/if}

<style>
  .mini {
    appearance: none;
    border: 1px solid var(--line-1);
    background: var(--bg-2);
    color: var(--fg-2);
    font-size: var(--fs-2xs);
    height: 18px;
    padding: 0 6px;
    cursor: pointer;
    border-radius: 3px;
  }
  .mini:hover {
    color: var(--fg-0);
  }
  .mini.x:hover {
    color: var(--bad);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .err {
    color: var(--bad);
  }

  .shape {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: 6px;
  }
  .srow {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
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
    background: none;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    cursor: pointer;
  }
  .chip.on {
    color: var(--fg-0);
    border-color: var(--fg-2);
    background: var(--bg-active);
  }
  .cruc {
    align-items: flex-start;
  }
  .crucnodes {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }
  .crucsel {
    width: 100%;
    max-width: 440px;
  }
  .clusterpick {
    flex: 1;
    min-width: 160px;
    max-width: 320px;
  }
  .socks {
    display: flex;
    align-items: center;
    gap: 3px;
    flex-wrap: wrap;
  }
  .sock {
    display: inline-flex;
    align-items: center;
  }
  .sockc {
    width: 78px;
  }
  .link {
    padding: 0 2px;
    background: none;
    border: 0;
    color: var(--fg-4);
    cursor: pointer;
  }
  .link.on {
    color: var(--fg-0);
  }
  .grow2 {
    flex: 1;
    min-width: 0;
  }
  .anointdlg {
    width: 520px;
  }
  .anointlist {
    max-height: 46vh;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line-0);
    border-radius: var(--r-1);
  }
  .arow {
    appearance: none;
    border: 0;
    border-bottom: 1px solid var(--line-0);
    background: none;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
    padding: 4px 8px;
    color: var(--fg-1);
    font-size: var(--fs-sm);
    text-align: left;
    cursor: pointer;
  }
  .arow:hover {
    background: var(--bg-2);
    color: var(--fg-0);
  }
  .arow.alloc .aname {
    color: var(--ok);
  }
  .rline {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  fieldset {
    border: 0;
    padding: 0;
    margin: 0;
    min-width: 0;
  }
  .modrow {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .modal {
    position: fixed;
    inset: 0;
    display: grid;
    place-items: center;
    background: var(--backdrop);
    z-index: 6;
  }
  .dialog {
    width: min(720px, 95vw);
    max-height: 85vh;
    overflow: auto;
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .crow {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }
  .range {
    flex: 1;
  }
  .clabel {
    color: var(--fg-2);
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 8px;
  }
</style>
