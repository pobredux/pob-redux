<script lang="ts">
  import { engine, type ConfigOption, type ConfigState, type CustomModBlock } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import PobText from "$lib/components/PobText.svelte";
  import { stripPobText } from "$lib/pobtext";
  import { untrack } from "svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { modKey } from "$lib/keys";
  import Kbd from "$lib/components/Kbd.svelte";
  import { m } from "$lib/paraglide/messages";
  import { configLabel } from "$lib/pob-labels";

  let options = $state<ConfigOption[]>([]);
  let config = $state<Record<string, unknown>>({});
  let placeholder = $state<Record<string, unknown>>({});
  let sets = $state<ConfigState["sets"]>([]);
  let visibility = $state<Record<string, boolean> | null>(null);
  let customBlocks = $state<CustomModBlock[]>([]);
  let relevantOnly = $state(true);
  let filter = $state("");
  const COLLAPSED_KEY = "pob-redux:config-collapsed";
  let collapsed = $state<Set<string>>(new Set());
  try {
    collapsed = new Set(JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]"));
  } catch {}
  let renamingSet = $state(false);
  let setDraft = $state("");

  let browser = $state<number | null>(null); // block index
  let browserMods = $state<{ text: string; sources: string[] }[] | null>(null);
  let browserQuery = $state("");

  $effect(() => {
    engine.listConfigOptions().then((r) => (options = r.options));
  });
  $effect(() => {
    build.rev;
    engine.getConfig().then((r) => {
      config = r.config;
      placeholder = r.placeholder;
      sets = r.sets;
    });
    engine.getCustomMods().then((r) => (customBlocks = r.blocks)).catch(() => (customBlocks = []));
    engine
      .configVisibility()
      .then((r) => (visibility = r.visibility))
      .catch(() => (visibility = null));
  });

  const activeSet = $derived(sets.find((s) => s.active));

  const sections = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const out: { name: string; items: ConfigOption[] }[] = [];
    for (const o of options) {
      if (q && !`${o.group ?? ""} ${o.label ?? ""} ${configLabel(o.label ?? "")} ${o.var}`.toLowerCase().includes(q)) continue;
      // Hide options PoB would hide for this build, but never hide one the user changed.
      if (relevantOnly && !q && visibility && !visibility[o.var] && !isChanged(o, config[o.var])) continue;
      const name = o.section ?? m.config_section_general();
      let s = out.find((x) => x.name === name);
      if (!s) out.push((s = { name, items: [] }));
      s.items.push(o);
    }
    return out;
  });

  const CARD_MIN = 380;
  const GAP = 14;
  let cardsW = $state(0);
  let heights = $state<Record<string, number>>({});

  // Placed by hand: WebKitGTK does not rebalance CSS columns when the cards arrive after first layout.
  const layout = $derived.by(() => {
    const cols = Math.max(1, Math.floor((cardsW + GAP) / (CARD_MIN + GAP)));
    const width = (cardsW - GAP * (cols - 1)) / cols;
    const hs = sections.map((s) => (heights[s.name] ?? 0) + GAP);
    const fits = (cap: number) => {
      let used = 1;
      let y = 0;
      for (const h of hs) {
        if (y && y + h > cap) {
          used++;
          y = 0;
        }
        y += h;
      }
      return used <= cols;
    };
    let lo = Math.max(0, ...hs);
    let hi = hs.reduce((a, b) => a + b, 0);
    while (lo < hi) {
      const mid = Math.floor((lo + hi) / 2);
      if (fits(mid)) hi = mid;
      else lo = mid + 1;
    }
    const pos: Record<string, { x: number; y: number }> = {};
    let col = 0;
    let y = 0;
    let bottom = 0;
    sections.forEach((s, i) => {
      if (y && y + hs[i] > lo) {
        col++;
        y = 0;
      }
      pos[s.name] = { x: col * (width + GAP), y };
      y += hs[i];
      bottom = Math.max(bottom, y);
    });
    return { width, pos, height: Math.max(0, bottom - GAP) };
  });

  function set(o: ConfigOption, value: unknown) {
    build.run(() => engine.setConfig(o.var, value));
  }
  function saveCollapsed(next: Set<string>) {
    collapsed = next;
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
    } catch {}
  }
  function toggle(name: string) {
    const s = new Set(collapsed);
    s.has(name) ? s.delete(name) : s.add(name);
    saveCollapsed(s);
  }
  const allNames = $derived(["__custom", ...sections.map((x) => x.name)]);
  const allCollapsed = $derived(allNames.every((n) => collapsed.has(n)));
  function toggleAll() {
    saveCollapsed(allCollapsed ? new Set() : new Set(allNames));
  }
  // PoB writes each option's default into the config, and an unticked box is stored as nil.
  const norm = (v: unknown) => (v === undefined || v === null || v === false ? null : String(v));
  const isChanged = (o: ConfigOption, v: unknown) => norm(v) !== norm(o.defaultState);

  // PoB pads some list labels with spaces and then search keywords, which its narrow dropdown clips off.
  function listLabel(label: string | null | undefined): string {
    return stripPobText(configLabel(stripPobText(label).split(/ {8,}/)[0].trim()));
  }

  function badLines(b: CustomModBlock) {
    return b.lines.filter((l) => l.status === "none" || l.status === "partial");
  }

  async function openBrowser(blockIndex: number) {
    browser = blockIndex;
    browserQuery = "";
    if (!browserMods) {
      try {
        browserMods = (await engine.customModBrowser()).mods;
      } catch {
        browserMods = [];
      }
    }
  }

  const browserList = $derived.by(() => {
    if (!browserMods) return [];
    const words = browserQuery.trim().toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) return browserMods.slice(0, 200);
    return browserMods.filter((mod) => {
      const t = mod.text.toLowerCase();
      return words.every((w) => t.includes(w));
    }).slice(0, 200);
  });

  function addModFromBrowser(text: string) {
    if (browser == null) return;
    const block = customBlocks.find((b) => b.index === browser);
    if (!block) return;
    const joined = block.text && !block.text.endsWith("\n") ? block.text + "\n" + text : (block.text ?? "") + text;
    build.run(() => engine.setCustomModBlock(block.index, { text: joined }));
  }

  let filterEl = $state<HTMLInputElement>();
  function onKey(e: KeyboardEvent) {
    if (modKey(e) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "f") {
      e.preventDefault();
      filterEl?.focus();
      filterEl?.select();
    }
  }

  $effect(() => {
    const j = ui.jump;
    if (j?.view !== "config" || !options.length) return;
    untrack(() => {
      ui.jump = null;
      filter = j.label;
      const o = options.find((x) => stripPobText(x.label ?? x.var).replace(/:$/, "") === j.label);
      const name = o?.section ?? m.config_section_general();
      if (collapsed.has(name)) saveCollapsed(new Set([...collapsed].filter((n) => n !== name)));
    });
  });
</script>

<svelte:window onkeydown={onKey} />

<div class="page">
  <div class="toolbar">
    {#if renamingSet}
      <input
        class="input setsel"
        bind:value={setDraft}
        onblur={() => {
          renamingSet = false;
          if (activeSet && setDraft.trim() && setDraft !== (activeSet.title ?? "")) build.run(() => engine.renameConfigSet(activeSet.id, setDraft.trim()));
        }}
        onkeydown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          if (e.key === "Escape") renamingSet = false;
        }}
      />
    {:else}
      <select
        class="select setsel"
        value={activeSet?.id ?? 1}
        onchange={(e) => build.run(() => engine.selectConfigSet(Number((e.target as HTMLSelectElement).value)))}
        disabled={build.busy > 0}
        title={m.config_set_title()}
      >
        {#each sets as s}
          <option value={s.id}>{stripPobText(s.title ?? m.config_set_default())}</option>
        {/each}
      </select>
    {/if}
    <button class="btn sm ghost" onclick={() => build.run(() => engine.createConfigSet())}>{m.common_new()}</button>
    <button class="btn sm ghost" onclick={() => build.run(() => engine.copyConfigSet())}>{m.common_copy_button()}</button>
    <button
      class="btn sm ghost"
      onclick={() => {
        setDraft = activeSet?.title ?? "";
        renamingSet = true;
      }}>{m.common_rename()}</button
    >
    <button class="btn sm ghost" disabled={sets.length <= 1} onclick={() => activeSet && build.run(() => engine.deleteConfigSet(activeSet.id))}>{m.common_delete()}</button>
    <span class="vr"></span>
    <span class="hinted">
      <input class="input" placeholder={m.config_filter()} bind:value={filter} bind:this={filterEl} />
      {#if !filter}<Kbd keys="Mod+F" hint />{/if}
    </span>
    <label class="chk small" title={m.config_relevant_only_title()}>
      <input type="checkbox" bind:checked={relevantOnly} disabled={!visibility} />
      {m.config_relevant_only()}
    </label>
    <button class="btn sm ghost" onclick={toggleAll}>{allCollapsed ? m.config_expand_all() : m.config_collapse_all()}</button>
    <span class="dim small count">
      {#if visibility}
        {m.config_apply_count({ shown: Object.values(visibility).filter(Boolean).length, total: options.length })}
      {/if}
    </span>
  </div>
  <div class="scroll">
    <section class="card custom">
      <button class="chead" aria-expanded={!collapsed.has("__custom")} onclick={() => toggle("__custom")}>
        <span class="caret" class:open={!collapsed.has("__custom")}>▸</span>
        <span class="cname">{m.config_custom_mods()}</span>
        <span class="ccount num">{customBlocks.length}</span>
      </button>
      {#if !collapsed.has("__custom")}
        <div class="blocks">
          {#each customBlocks as b (b.index)}
            <div class="block" class:off={!b.enabled}>
              <div class="bhead">
                <input
                  type="checkbox"
                  checked={b.enabled}
                  title={m.config_block_enable()}
                  onchange={(e) => build.run(() => engine.setCustomModBlock(b.index, { enabled: (e.target as HTMLInputElement).checked }))}
                />
                <input
                  class="input btitle"
                  value={b.title ?? ""}
                  placeholder={m.config_block_name()}
                  onchange={(e) => build.run(() => engine.setCustomModBlock(b.index, { title: (e.target as HTMLInputElement).value }))}
                />
                <button class="btn sm ghost" onclick={() => openBrowser(b.index)} title={m.config_browse_title()}>{m.config_browse()}</button>
                <button class="btn sm ghost" disabled={customBlocks.length <= 1 && !b.text} onclick={() => build.run(() => engine.deleteCustomModBlock(b.index))}>{m.common_delete()}</button>
              </div>
              <textarea
                class="bmods mono"
                rows={Math.max(3, b.lines.length + 1)}
                value={b.text}
                placeholder={m.config_mods_placeholder()}
                spellcheck="false"
                onchange={(e) => build.run(() => engine.setCustomModBlock(b.index, { text: (e.target as HTMLTextAreaElement).value }))}
              ></textarea>
              {#if badLines(b).length}
                <div class="bstatus">
                  {#each badLines(b) as l}
                    <div class="bline" class:partial={l.status === "partial"}>
                      <span class="mark">{l.status === "partial" ? "~" : "✕"}</span>
                      <span class="mono"><PobText text={l.text.trim()} /></span>
                      <span class="dim">{l.status === "partial" ? m.config_line_partial() : m.config_line_unknown()}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            </div>
          {/each}
          <button class="btn sm ghost addblock" onclick={() => build.run(() => engine.addCustomModBlock())}>{m.config_add_block()}</button>
        </div>
      {/if}
    </section>

    {#if sections.length === 0}
      <p class="dim small empty">{m.config_no_match()}</p>
    {/if}
    <div class="cards" bind:clientWidth={cardsW} style:height={`${layout.height}px`}>
      {#each sections as sec (sec.name)}
        {@const changed = sec.items.filter((o) => isChanged(o, config[o.var])).length}
        {@const p = layout.pos[sec.name]}
        <section class="card" bind:offsetHeight={heights[sec.name]} style:width={`${layout.width}px`} style:left={`${p.x}px`} style:top={`${p.y}px`}>
          <button class="chead" aria-expanded={!collapsed.has(sec.name)} onclick={() => toggle(sec.name)}>
            <span class="caret" class:open={!collapsed.has(sec.name)}>▸</span>
            <span class="cname">{configLabel(sec.name)}</span>
            {#if changed}<span class="cset num">{m.config_set_count({ count: changed })}</span>{/if}
            <span class="ccount num">{sec.items.length}</span>
          </button>
          {#if !collapsed.has(sec.name)}
            <div class="items">
              {#each sec.items as o, i (o.var)}
                {@const v = config[o.var]}
                {#if o.group && o.group !== sec.items[i - 1]?.group}
                  <div class="ogroup"><PobText text={configLabel(o.group.replace(/:\s*$/, ""))} /></div>
                {/if}
                <label class="opt" class:set={isChanged(o, v)} title={o.tooltip ?? undefined}>
                  <span class="olabel"><PobText text={configLabel(o.label ?? o.var)} /></span>
                  {#if o.type === "check"}
                    <input type="checkbox" checked={v === true} onchange={(e) => set(o, (e.target as HTMLInputElement).checked ? true : null)} />
                  {:else if o.type === "list" && o.list}
                    <select class="select sm" value={v ?? ""} onchange={(e) => { const raw = (e.target as HTMLSelectElement).value; const opt = o.list!.find((x) => String(x.val ?? "") === raw); set(o, opt ? opt.val : null); }}>
                      <option value="">{placeholder[o.var] != null ? `(${listLabel(o.list.find((x) => String(x.val) === String(placeholder[o.var]))?.label) || placeholder[o.var]})` : "—"}</option>
                      {#each o.list as e}
                        <option value={String(e.val ?? "")}>{listLabel(e.label)}</option>
                      {/each}
                    </select>
                  {:else if o.type === "count" || o.type === "integer" || o.type === "countAllowZero"}
                    <input class="input sm num" type="number" value={v ?? ""} placeholder={placeholder[o.var] != null ? String(placeholder[o.var]) : ""} onchange={(e) => { const s = (e.target as HTMLInputElement).value; set(o, s === "" ? null : Number(s)); }} />
                  {:else}
                    <input class="input sm" type="text" value={v ?? ""} placeholder={placeholder[o.var] != null ? String(placeholder[o.var]) : ""} onchange={(e) => { const s = (e.target as HTMLInputElement).value; set(o, s === "" ? null : s); }} />
                  {/if}
                </label>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  </div>
</div>

{#if browser != null}
  <div class="overlay" role="presentation" onclick={() => (browser = null)} onkeydown={(e) => e.key === "Escape" && (browser = null)}>
    <div class="modal" role="dialog" aria-label={m.config_browser_title()} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (browser = null)}>
      <div class="mhead">
        <span class="label">{m.config_browser_title()}</span>
        <button class="btn sm ghost" onclick={() => (browser = null)}>{m.common_close()}</button>
      </div>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input msearch" placeholder={m.config_browser_search()} bind:value={browserQuery} autofocus />
      <div class="mlist">
        {#if !browserMods}
          <div class="dim small pad">{m.common_loading()}</div>
        {:else if !browserList.length}
          <div class="dim small pad">{m.config_browser_none()}</div>
        {:else}
          {#each browserList as mod}
            <button class="mrow" title={mod.sources.join(", ")} onclick={() => addModFromBrowser(mod.text)}>{mod.text}</button>
          {/each}
          {#if browserMods.length > browserList.length && !browserQuery.trim()}
            <div class="dim small pad">{m.config_browser_more({ count: browserMods.length - browserList.length })}</div>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-1);
  }
  .toolbar .input {
    width: 220px;
  }
  .hinted {
    position: relative;
    display: flex;
  }
  .setsel {
    width: 160px;
  }
  .vr {
    width: 1px;
    height: 16px;
    background: var(--line-1);
    margin: 0 4px;
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 14px;
  }
  .count {
    margin-left: auto;
  }
  .empty {
    margin: 4px 2px 14px;
  }
  .cards {
    position: relative;
  }
  .cards > .card {
    position: absolute;
  }
  .card {
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow: clip;
  }
  .card.custom {
    margin-bottom: 14px;
  }
  .chead {
    appearance: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 12px;
    border: 0;
    border-bottom: 1px solid var(--line-1);
    background: var(--bg-3);
    color: var(--fg-0);
    text-align: left;
  }
  .chead[aria-expanded="false"] {
    border-bottom: 0;
  }
  .chead:hover {
    background: var(--bg-hover);
  }
  .cname {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .ccount {
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .cset {
    padding: 1px 7px;
    border-radius: 999px;
    background: color-mix(in oklab, var(--focus) var(--sel-mix), var(--bg-1));
    color: var(--fg-0);
    font-size: var(--fs-2xs);
  }
  .caret {
    display: inline-block;
    width: 10px;
    color: var(--fg-2);
    transition: transform 100ms;
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .blocks {
    display: flex;
    flex-wrap: wrap;
    gap: 10px;
    padding: 10px 12px;
    align-items: flex-start;
  }
  .block {
    width: 380px;
    background: var(--bg-0);
    border: 1px solid var(--line-0);
    border-radius: var(--r-2);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .block.off {
    opacity: 0.55;
  }
  .bhead {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .btitle {
    flex: 1;
    height: 22px;
    font-size: var(--fs-xs);
  }
  .bmods {
    background: var(--bg-0);
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    color: var(--fg-0);
    font-size: var(--fs-xs);
    line-height: 1.5;
    padding: 6px 8px;
    resize: vertical;
    min-height: 60px;
  }
  .bmods:focus {
    outline: none;
    border-color: var(--focus);
  }
  .bstatus {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--fs-xs);
  }
  .bline {
    display: flex;
    gap: 6px;
    align-items: baseline;
    color: var(--bad);
  }
  .bline.partial {
    color: var(--warn);
  }
  .bline .mark {
    width: 10px;
    text-align: center;
  }
  .bline .dim {
    margin-left: auto;
  }
  .addblock {
    align-self: flex-start;
  }
  .items {
    display: flex;
    flex-direction: column;
  }
  .opt {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 12px;
    padding: 5px 12px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
    color: var(--fg-2);
    min-height: 32px;
    cursor: pointer;
  }
  .opt:last-child {
    border-bottom: 0;
  }
  .opt:hover {
    background: var(--bg-hover);
    color: var(--fg-1);
  }
  .opt.set {
    color: var(--fg-0);
    box-shadow: inset 2px 0 0 var(--focus);
  }
  .opt input[type="text"],
  .opt input[type="number"] {
    cursor: text;
  }
  .ogroup {
    padding: 10px 12px 2px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .olabel {
    min-width: 0;
    line-height: 1.3;
    overflow-wrap: anywhere;
  }
  .sm {
    height: 22px;
    font-size: var(--fs-xs);
  }
  .input.sm {
    flex: none;
    width: 90px;
    text-align: right;
  }
  .select.sm {
    flex: none;
    max-width: 180px;
  }
  .small {
    font-size: var(--fs-xs);
  }
  input[type="checkbox"] {
    accent-color: var(--fg-0);
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--fg-1);
    white-space: nowrap;
  }
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--backdrop);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 640px;
    max-width: 90vw;
    max-height: 78vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-modal);
  }
  .mhead {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line-0);
  }
  .mhead .btn {
    margin-left: auto;
  }
  .msearch {
    margin: 10px 12px 0;
  }
  .mlist {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: 8px 6px 10px;
  }
  .mrow {
    appearance: none;
    border: 0;
    background: none;
    text-align: left;
    padding: 3px 8px;
    font-size: var(--fs-xs);
    color: var(--c-magic);
    cursor: pointer;
    border-radius: 3px;
    white-space: normal;
  }
  .mrow:hover {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .pad {
    padding: 6px 10px;
  }
</style>
