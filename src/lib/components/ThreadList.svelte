<script lang="ts">
  import { tick } from "svelte";
  import { chat, type ThreadMeta } from "$lib/state/chat.svelte";
  import { locale } from "$lib/state/locale.svelte";
  import Icon from "./Icon.svelte";
  import WorkingTimer from "./WorkingTimer.svelte";
  import { m } from "$lib/paraglide/messages";

  let { onpick, autofocus = false }: { onpick?: () => void; autofocus?: boolean } = $props();

  let query = $state("");
  let renaming = $state<string | null>(null);
  let draft = $state("");
  let deleting = $state<string | null>(null);
  let search = $state<HTMLInputElement>();

  $effect(() => {
    if (autofocus) search?.focus();
  });

  const fresh = $derived(chat.turns.length === 0);
  const rows = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    const all = chat.threads.filter((t) => !needle || t.title.toLowerCase().includes(needle) || t.preview.toLowerCase().includes(needle));
    return { pinned: all.filter((t) => t.pinned), recent: all.filter((t) => !t.pinned) };
  });

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
</script>

{#snippet row(t: ThreadMeta)}
  {@const open = t.id === chat.thread.id}
  {@const locked = chat.busy && !open}
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
          if (e.key === "Escape") renaming = null;
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
      <button class="pick" disabled={locked} title={locked ? m.threads_busy() : t.title} onclick={() => pick(t)} ondblclick={() => beginRename(t)}>
        <span class="t1">
          {#if t.pinned}<span class="pinned"><Icon name="push-pin" size={10} /></span>{/if}
          <span class="name">{t.title || m.threads_new()}</span>
        </span>
        <span class="t2">
          {#if open && chat.busy && chat.startedAt}
            <span class="working">{m.threads_working()} <WorkingTimer since={chat.startedAt} /></span>
          {:else}
            <span>{ago(t.updatedAt)}</span>
          {/if}
          {#if t.build}<span class="sep">·</span><span class="build">{t.build}</span>{/if}
        </span>
      </button>
      <div class="hover">
        <button class="mini" title={t.pinned ? m.threads_unpin() : m.threads_pin()} aria-label={t.pinned ? m.threads_unpin() : m.threads_pin()} onclick={() => chat.togglePin(t.id)}>
          <Icon name="push-pin" size={12} />
        </button>
        <button class="mini" title={m.threads_rename()} aria-label={m.threads_rename()} onclick={() => beginRename(t)}>
          <Icon name="note-pencil" size={12} />
        </button>
        <button class="mini" title={m.threads_delete()} aria-label={m.threads_delete()} disabled={open && chat.busy} onclick={() => (deleting = t.id)}>
          <Icon name="trash" size={12} />
        </button>
      </div>
    {/if}
  </div>
{/snippet}

<div class="threads">
  <div class="top">
    <input class="input search" type="search" bind:this={search} bind:value={query} placeholder={m.threads_search()} aria-label={m.threads_search()} />
    <button class="btn sm" disabled={chat.busy} title={chat.busy ? m.threads_busy() : m.threads_new_title()} onclick={startNew}>
      <Icon name="plus" size={12} />
      {m.threads_new()}
    </button>
  </div>
  <div class="list" role="list" aria-label={m.threads_title()}>
    {#if fresh && !query}
      <div class="row open" role="listitem">
        <div class="pick static">
          <span class="t1"><span class="name">{chat.thread.title || m.threads_new()}</span></span>
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
</div>

<style>
  .threads {
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
  }
  .top {
    display: flex;
    gap: 6px;
    padding: 8px;
    border-bottom: 1px solid var(--line-1);
  }
  .search {
    flex: 1;
    min-width: 0;
  }
  .top .btn {
    flex: none;
    display: inline-flex;
    align-items: center;
    gap: 5px;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px;
    scrollbar-width: thin;
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
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 8px;
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
