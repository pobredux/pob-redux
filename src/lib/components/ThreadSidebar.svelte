<script lang="ts">
  import { scrollHint } from "$lib/scroll-hint";
  import { tick } from "svelte";
  import { chat, type ThreadMeta } from "$lib/state/chat.svelte";
  import { locale } from "$lib/state/locale.svelte";
  import Icon from "./Icon.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import type { MenuItem } from "./MoreMenu.svelte";
  import WorkingTimer from "./WorkingTimer.svelte";
  import { m } from "$lib/paraglide/messages";

  let {
    full,
    floating = false,
    ontoggle,
    onpick,
  }: { full: boolean; floating?: boolean; ontoggle: () => void; onpick?: () => void } = $props();

  let query = $state("");
  let renaming = $state<string | null>(null);
  let draft = $state("");
  let deleting = $state<string | null>(null);
  let search = $state<HTMLInputElement>();
  let menu = $state<{ x: number; y: number; thread: ThreadMeta } | null>(null);

  $effect(() => {
    if (floating) search?.focus();
  });

  const fresh = $derived(chat.turns.length === 0);
  const waiting = $derived(chat.turns.some((t) => t.kind === "tool" && t.status === "awaiting"));

  const split = (list: ThreadMeta[]) => ({ pinned: list.filter((t) => t.pinned), recent: list.filter((t) => !t.pinned) });
  const all = $derived(split(chat.threads));
  const rows = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return all;
    return split(chat.threads.filter((t) => t.title.toLowerCase().includes(needle) || t.preview.toLowerCase().includes(needle)));
  });

  const status = (t: ThreadMeta) => (t.id !== chat.thread.id || !chat.busy ? null : waiting ? "wait" : "busy");

  function monogram(title: string) {
    const [a = "", b = ""] = title.split(/[^\p{L}\p{N}]+/u).filter(Boolean);
    const chars = b ? [Array.from(a)[0], Array.from(b)[0]] : Array.from(a).slice(0, 2);
    return chars.join("").toLocaleUpperCase(locale.current) || "·";
  }

  // Keyed on the id, so a rename keeps the colour.
  function hue(id: string) {
    let h = 2166136261;
    for (let i = 0; i < id.length; i++) h = Math.imul(h ^ id.charCodeAt(i), 16777619);
    return ((h >>> 0) % 12) * 30 + 15;
  }

  function tip(t: ThreadMeta) {
    const st = status(t);
    if (chat.busy && t.id !== chat.thread.id) return m.threads_busy();
    return st === "wait" ? `${t.title} · ${m.threads_needs_you()}` : st === "busy" ? `${t.title} · ${m.threads_working()}` : t.title;
  }

  const ago = (ts: number) => {
    const fmt = new Intl.RelativeTimeFormat(locale.current, { numeric: "auto", style: "short" });
    const s = (ts - Date.now()) / 1000;
    for (const [unit, size] of [["year", 31_536_000], ["month", 2_592_000], ["week", 604_800], ["day", 86_400], ["hour", 3_600], ["minute", 60]] as const) {
      if (Math.abs(s) >= size) return fmt.format(Math.round(s / size), unit);
    }
    return fmt.format(0, "minute");
  };

  async function pick(t: ThreadMeta) {
    if (renaming || deleting) return;
    await chat.openThread(t.id);
    onpick?.();
  }

  async function startNew() {
    await chat.startThread();
    onpick?.();
  }

  async function beginRename(t: ThreadMeta) {
    if (!full) ontoggle();
    deleting = null;
    renaming = t.id;
    draft = t.title;
    await tick();
    document.querySelector<HTMLInputElement>(`[data-rename="${t.id}"]`)?.select();
  }

  async function commitRename() {
    const id = renaming;
    renaming = null;
    if (id) await chat.renameThread(id, draft);
  }

  function beginDelete(t: ThreadMeta) {
    if (!full) ontoggle();
    renaming = null;
    deleting = t.id;
  }

  function openMenu(e: MouseEvent, thread: ThreadMeta) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, thread };
  }

  const menuItems = $derived.by((): MenuItem[] => {
    if (!menu) return [];
    const t = menu.thread;
    return [
      { label: t.pinned ? m.threads_unpin() : m.threads_pin(), onclick: () => void chat.togglePin(t.id) },
      { label: m.threads_rename(), onclick: () => void beginRename(t) },
      { label: m.threads_delete(), danger: true, separator: true, disabled: t.id === chat.thread.id && chat.busy, onclick: () => beginDelete(t) },
    ];
  });
</script>

{#snippet mark(t: ThreadMeta | null)}
  {@const st = t && status(t)}
  <span class="mono" class:open={t ? t.id === chat.thread.id : fresh} class:new={!t} class:hued={!!t} style:--h={t ? hue(t.id) : null}>
    {#if t}{monogram(t.title)}{:else}<Icon name="plus" size={13} />{/if}
    {#if st}<span class="dot {st}"></span>{/if}
  </span>
{/snippet}

{#snippet tile(t: ThreadMeta)}
  {@const open = t.id === chat.thread.id}
  <button
    class="tile"
    class:open
    disabled={chat.busy && !open}
    aria-current={open ? "true" : undefined}
    aria-label={t.title}
    title={tip(t)}
    onclick={() => pick(t)}
    oncontextmenu={(e) => openMenu(e, t)}
  >
    {@render mark(t)}
  </button>
{/snippet}

{#snippet row(t: ThreadMeta)}
  {@const open = t.id === chat.thread.id}
  {@const locked = chat.busy && !open}
  {@const st = status(t)}
  <div class="row" class:open class:locked role="listitem">
    {#if renaming === t.id}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="input rename"
        data-rename={t.id}
        bind:value={draft}
        autofocus
        aria-label={m.threads_rename()}
        onblur={commitRename}
        onkeydown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          if (e.key === "Escape") {
            e.preventDefault();
            renaming = null;
          }
        }}
      />
    {:else if deleting === t.id}
      <div class="confirm">
        <span>{m.threads_delete_confirm()}</span>
        <div class="cacts">
          <button class="btn sm danger" onclick={() => ((deleting = null), chat.deleteThread(t.id))}>{m.threads_delete()}</button>
          <button class="btn sm ghost" onclick={() => (deleting = null)}>{m.common_cancel()}</button>
        </div>
      </div>
    {:else}
      <button
        class="pick"
        disabled={locked}
        aria-current={open ? "true" : undefined}
        title={tip(t)}
        onclick={() => pick(t)}
        ondblclick={() => beginRename(t)}
        oncontextmenu={(e) => openMenu(e, t)}
      >
        {@render mark(t)}
        <span class="text">
          <span class="t1">
            {#if t.pinned}<span class="pinned"><Icon name="push-pin" size={10} /></span>{/if}
            <span class="name">{t.title || m.threads_new()}</span>
          </span>
          <span class="t2">
            {#if st === "wait"}
              <span class="needs">{m.threads_needs_you()}</span>
            {:else if st === "busy" && chat.startedAt}
              <span class="working">{m.threads_working()} <WorkingTimer since={chat.startedAt} /></span>
            {:else}
              <span>{ago(t.updatedAt)}</span>
            {/if}
            {#if t.build}<span class="sep">·</span><span class="build">{t.build}</span>{/if}
          </span>
        </span>
      </button>
      <div class="hover">
        <button class="mini" title={t.pinned ? m.threads_unpin() : m.threads_pin()} aria-label={t.pinned ? m.threads_unpin() : m.threads_pin()} onclick={() => chat.togglePin(t.id)}>
          <Icon name="push-pin" size={12} />
        </button>
        <button class="mini" title={m.threads_rename()} aria-label={m.threads_rename()} onclick={() => beginRename(t)}>
          <Icon name="note-pencil" size={12} />
        </button>
        <button class="mini" title={m.threads_delete()} aria-label={m.threads_delete()} disabled={open && chat.busy} onclick={() => beginDelete(t)}>
          <Icon name="trash" size={12} />
        </button>
      </div>
    {/if}
  </div>
{/snippet}

<nav class="side" class:full class:floating aria-label={m.threads_title()}>
  <div class="top">
    <button
      class="icon"
      class:on={full}
      aria-expanded={full}
      title={`${full ? m.threads_collapse() : m.threads_expand()} · ${m.threads_menu_title()}`}
      aria-label={full ? m.threads_collapse() : m.threads_expand()}
      onclick={ontoggle}
    >
      <Icon name="sidebar-simple" size={16} />
    </button>
    {#if full}
      <span class="label">{m.threads_title()}</span>
      <button class="icon fresh" disabled={chat.busy} title={chat.busy ? m.threads_busy() : m.threads_new_title()} aria-label={m.threads_new()} onclick={startNew}>
        <Icon name="plus-square" size={17} />
      </button>
    {/if}
  </div>

  {#if full}
    <div class="find">
      <input class="input search" type="search" bind:this={search} bind:value={query} placeholder={m.threads_search()} aria-label={m.threads_search()} />
    </div>
    <div class="list scroll-hint" {@attach scrollHint} role="list" aria-label={m.threads_title()}>
      {#if fresh && !query}
        <div class="row open" role="listitem">
          <div class="pick static">
            {@render mark(null)}
            <span class="text"><span class="t1"><span class="name">{chat.thread.title || m.threads_new()}</span></span></span>
          </div>
        </div>
      {/if}
      {#if rows.pinned.length}
        <div class="sec">{m.threads_pinned()}</div>
        {#each rows.pinned as t (t.id)}{@render row(t)}{/each}
      {/if}
      {#if rows.recent.length}
        {#if rows.pinned.length}<div class="sec">{m.threads_recent()}</div>{/if}
        {#each rows.recent as t (t.id)}{@render row(t)}{/each}
      {/if}
      {#if !chat.threads.length}
        <p class="empty">{m.threads_empty()}</p>
      {:else if query && !rows.pinned.length && !rows.recent.length}
        <p class="empty">{m.threads_no_match()}</p>
      {/if}
    </div>
  {:else}
    <button class="tile add" class:open={fresh} disabled={chat.busy} title={chat.busy ? m.threads_busy() : m.threads_new_title()} aria-label={m.threads_new()} onclick={startNew}>
      {@render mark(null)}
    </button>
    <div class="tiles scroll-hint" {@attach scrollHint} role="list" aria-label={m.threads_title()}>
      {#each all.pinned as t (t.id)}{@render tile(t)}{/each}
      {#if all.pinned.length && all.recent.length}<div class="rule" role="separator"></div>{/if}
      {#each all.recent as t (t.id)}{@render tile(t)}{/each}
    </div>
  {/if}
</nav>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} label={m.threads_menu()} items={menuItems} onclose={() => (menu = null)} />
{/if}

<style>
  .side {
    display: flex;
    flex-direction: column;
    flex: none;
    width: 44px;
    min-height: 0;
    height: 100%;
    background: var(--bg-0);
  }
  .side.full {
    width: 240px;
  }
  .side.floating {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    z-index: 20;
    width: 264px;
    border-right: 1px solid var(--line-2);
    box-shadow: var(--shadow-pop);
  }
  .top {
    display: flex;
    flex: none;
    align-items: center;
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid var(--line-1);
  }
  .side:not(.full) .top {
    justify-content: center;
    padding: 8px 0;
  }
  .label {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--fg-1);
  }
  .icon {
    display: grid;
    flex: none;
    place-items: center;
    width: 28px;
    height: 26px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--r-1);
    background: none;
    color: var(--fg-2);
    cursor: var(--cursor-pointer, pointer);
  }
  .icon:hover:not(:disabled),
  .icon.on {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .icon.fresh:hover:not(:disabled) {
    color: var(--focus);
  }
  .icon:disabled {
    opacity: var(--fade-off);
  }

  .mono {
    position: relative;
    display: grid;
    flex: none;
    place-items: center;
    width: 28px;
    height: 28px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    background: var(--bg-2);
    color: var(--fg-2);
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    font-weight: 600;
    letter-spacing: 0.02em;
  }
  .mono.new {
    border-style: dashed;
    background: none;
  }
  .mono.open {
    border-color: var(--line-2);
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .mono.hued {
    --tag: oklch(var(--c-thread-l) var(--c-thread-c) var(--h));
    border-color: color-mix(in oklab, var(--tag) 35%, var(--bg-1));
    background: color-mix(in oklab, var(--tag) 14%, var(--bg-1));
    color: var(--tag);
  }
  .mono.hued.open {
    border-color: var(--tag);
    background: color-mix(in oklab, var(--tag) 30%, var(--bg-1));
    color: color-mix(in oklab, var(--tag) 45%, var(--fg-0));
  }
  .dot {
    position: absolute;
    top: -3px;
    right: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    box-shadow: 0 0 0 2px var(--bg-0);
  }
  .dot.busy {
    background: var(--fg-1);
    animation: pulse 1.2s ease-in-out infinite;
  }
  .dot.wait {
    background: var(--warn);
  }

  .tile {
    position: relative;
    display: grid;
    flex: none;
    place-items: center;
    width: 44px;
    padding: 3px 0;
    border: 0;
    background: none;
    cursor: var(--cursor-pointer, pointer);
  }
  .tile.add {
    margin-top: 6px;
  }
  .tile:hover:not(:disabled) .mono {
    border-color: var(--line-2);
    color: var(--fg-0);
  }
  .tile:hover:not(:disabled) .mono.hued {
    border-color: var(--tag);
    color: color-mix(in oklab, var(--tag) 45%, var(--fg-0));
  }
  .tile:focus-visible {
    outline: none;
  }
  .tile:focus-visible .mono {
    border-color: var(--focus);
  }
  .tile:disabled {
    cursor: var(--cursor-arrow, default);
  }
  .tile:disabled .mono {
    opacity: var(--fade-off);
  }
  .tile.open::before {
    content: "";
    position: absolute;
    top: 9px;
    bottom: 9px;
    left: 0;
    width: 2px;
    border-radius: 0 2px 2px 0;
    background: var(--fg-0);
  }
  .tiles {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    padding: 4px 0 6px;
    overflow-x: hidden;
    overflow-y: auto;
    scrollbar-width: none;
  }
  .rule {
    flex: none;
    width: 20px;
    height: 1px;
    margin: 5px auto;
    background: var(--line-2);
  }

  .find {
    flex: none;
    padding: 8px 8px 4px;
  }
  .search {
    width: 100%;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px;
  }
  .sec {
    padding: 8px 8px 3px;
    font-size: var(--fs-2xs);
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--fg-3);
  }
  .row {
    position: relative;
    border-radius: var(--r-1);
  }
  .row:hover,
  .row:focus-within {
    background: var(--bg-hover);
  }
  .row.open {
    background: var(--bg-active);
  }
  .pick {
    appearance: none;
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 6px;
    border: 0;
    background: none;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
  }
  .pick.static {
    cursor: var(--cursor-arrow, default);
  }
  .row.open .pick {
    color: var(--fg-0);
  }
  .row.locked .pick {
    opacity: 0.55;
  }
  .row:hover .pick:not(.static),
  .row:focus-within .pick:not(.static) {
    padding-right: 84px;
  }
  .text {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .t1 {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    font-size: var(--fs-sm);
  }
  .name,
  .build {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pinned {
    display: grid;
    flex: none;
    color: var(--fg-3);
  }
  .t2 {
    display: flex;
    align-items: baseline;
    min-width: 0;
    font-size: var(--fs-xs);
    color: var(--fg-3);
  }
  .t2 > span {
    flex: none;
  }
  .t2 .build {
    flex: 0 1 auto;
  }
  .sep {
    margin: 0 4px;
  }
  .working {
    color: var(--fg-1);
  }
  .needs {
    color: var(--warn);
  }
  .hover {
    position: absolute;
    top: 50%;
    right: 6px;
    display: none;
    gap: 2px;
    translate: 0 -50%;
  }
  .row:hover .hover,
  .row:focus-within .hover {
    display: flex;
  }
  .mini {
    appearance: none;
    display: grid;
    place-items: center;
    width: 24px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: var(--r-1);
    background: transparent;
    color: var(--fg-2);
  }
  .mini:hover:not(:disabled) {
    background: var(--bg-1);
    color: var(--fg-0);
  }
  .mini:disabled {
    opacity: 0.4;
  }
  .rename {
    width: calc(100% - 8px);
    margin: 4px;
  }
  .confirm {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
  .cacts {
    display: flex;
    gap: 6px;
  }
  .danger {
    color: var(--bad);
  }
  .empty {
    margin: 0;
    padding: 14px 10px;
    font-size: var(--fs-xs);
    color: var(--fg-3);
  }
</style>
