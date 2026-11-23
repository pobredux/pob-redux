<script lang="ts">
  import { scrollHint } from "$lib/scroll-hint";
  import { tick, untrack, type Snippet } from "svelte";
  import { m } from "$lib/paraglide/messages";
  import Icon from "$lib/components/Icon.svelte";

  let { value, options, label, disabled = false, onchange, onhover, aside, lead }: {
    value: string;
    options: { value: string; label: string; color?: string; detail?: string }[];
    label: string;
    disabled?: boolean;
    onchange: (value: string) => void;
    onhover?: (value: string | null, anchor: HTMLElement | null) => void;
    aside?: Snippet<[string]>;
    lead?: Snippet<[string]>;
  } = $props();

  const ASIDE_WIDTH = 260;

  let open = $state(false);
  let query = $state("");
  let active = $state(0);
  let trigger = $state<HTMLButtonElement | null>(null);
  let pop = $state<HTMLDivElement | null>(null);
  let place = $state({ left: 0, top: 0, width: 0, up: false });
  const listId = `ss-${Math.random().toString(36).slice(2, 9)}`;

  const current = $derived(options.find((o) => o.value === value));
  // Every word has to appear somewhere, so "cold" finds "#% increased Cold Damage".
  const shown = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    return words.length ? options.filter((o) => words.every((w) => `${o.label} ${o.detail ?? ""}`.toLowerCase().includes(w))) : options;
  });

  async function show(initial = "") {
    if (disabled || !trigger) return;
    const r = trigger.getBoundingClientRect();
    const up = r.bottom + 320 > window.innerHeight && r.top > window.innerHeight - r.bottom;
    const width = Math.max(r.width, 320) + (aside ? ASIDE_WIDTH : 0);
    place = { left: Math.max(8, Math.min(r.left, window.innerWidth - width - 8)), top: up ? r.top : r.bottom, width, up };
    query = initial;
    active = Math.max(0, options.findIndex((o) => o.value === value));
    open = true;
    await tick();
    document.getElementById(`${listId}-${active}`)?.scrollIntoView({ block: "nearest" });
  }

  function close(refocus = false) {
    open = false;
    if (refocus) trigger?.focus();
  }

  function pick(o: { value: string }) {
    close(true);
    if (o.value !== value) onchange(o.value);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = shown.length;
      if (n) active = (active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      document.getElementById(`${listId}-${active}`)?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (shown[active]) pick(shown[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close(true);
    } else if (e.key === "Tab") {
      close();
    }
  }

  function onTriggerKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      void show();
    } else if (e.key.length === 1 && !e.ctrlKey && !e.metaKey && !e.altKey) {
      e.preventDefault();
      void show(e.key);
    }
  }

  function outside(e: PointerEvent) {
    if (open && !pop?.contains(e.target as Node) && !trigger?.contains(e.target as Node)) close();
  }

  let hovering = false;
  function hover(i: number | null) {
    const o = i == null ? undefined : shown[i];
    if (!o && !hovering) return;
    hovering = !!o;
    onhover?.(o?.value ?? null, o ? document.getElementById(`${listId}-${i}`) : null);
  }

  $effect(() => {
    const i = active;
    const o = open ? shown[i] : undefined;
    untrack(() => hover(o ? i : null));
  });
</script>

<svelte:window onpointerdown={outside} onresize={() => close()} />
<svelte:document onscrollcapture={(e) => open && !pop?.contains(e.target as Node) && close()} />

<button
  bind:this={trigger}
  type="button"
  class="select trigger"
  class:led={lead}
  aria-label={label}
  aria-haspopup="listbox"
  aria-expanded={open}
  title={current?.label}
  style:color={current?.color}
  {disabled}
  onclick={() => (open ? close() : show())}
  onkeydown={onTriggerKey}
>
  {#if lead && current}{@render lead(current.value)}{/if}
  <span class="text">{current?.label ?? ""}</span>
</button>
{#if open}
  <div
    bind:this={pop}
    class="pop"
    class:up={place.up}
    style:left={`${place.left}px`}
    style:top={place.up ? undefined : `${place.top + 4}px`}
    style:bottom={place.up ? `${window.innerHeight - place.top + 4}px` : undefined}
    style:width={`${place.width}px`}
  >
    <input
      class="input search"
      {@attach (el) => el.focus()}
      bind:value={query}
      placeholder={m.common_search()}
      aria-label={label}
      aria-controls={listId}
      aria-activedescendant={shown[active] ? `${listId}-${active}` : undefined}
      onkeydown={onKey}
      oninput={() => (active = 0)}
    />
    <div class="body">
      <div class="list scroll-hint" {@attach scrollHint} id={listId} role="listbox" aria-label={label}>
        {#each shown as o, i (o.value)}
          <button
            type="button"
            id={`${listId}-${i}`}
            role="option"
            aria-selected={o.value === value}
            class="opt"
            class:led={lead}
            class:hot={i === active}
            class:on={o.value === value}
            tabindex="-1"
            style:color={o.color}
            onpointerenter={() => hover(i)}
            onpointermove={() => (active = i)}
            onclick={() => pick(o)}
          >
            {#if lead}
              {@render lead(o.value)}
              <span class="olabel">{o.label}</span>
              {#if o.detail}<span class="detail">{o.detail}</span>{/if}
              <span class="check">{#if o.value === value}<Icon name="check" size={11} />{/if}</span>
            {:else}
              {o.label}
            {/if}
          </button>
        {:else}
          <div class="none">{m.common_no_matches()}</div>
        {/each}
      </div>
      {#if aside}
        <div class="aside" style:width={`${ASIDE_WIDTH - 4}px`}>{@render aside(shown[active]?.value ?? value)}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .trigger {
    width: 100%;
    overflow: hidden;
    text-align: left;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
  }
  .trigger.led,
  .opt.led {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .trigger.led .text,
  .opt.led .olabel {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .opt.led {
    padding: 6px 8px;
  }
  .opt.led .olabel {
    flex: 1;
  }
  .opt .detail {
    color: var(--fg-2);
    font-weight: 400;
    white-space: nowrap;
  }
  .opt .check {
    display: grid;
    place-items: center;
    width: 12px;
    flex: none;
    color: var(--fg-0);
  }
  .pop {
    position: fixed;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 4px;
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
  }
  .search {
    height: 24px;
    font-size: var(--fs-xs);
  }
  .body {
    display: flex;
    gap: 4px;
  }
  .list {
    flex: 1;
    min-width: 0;
    max-height: 280px;
    overflow-y: auto;
  }
  .aside {
    flex: none;
    border-left: 1px solid var(--line-1);
    padding-left: 4px;
  }
  .opt {
    display: block;
    width: 100%;
    padding: 4px 6px;
    background: none;
    border: 0;
    border-radius: var(--r-1);
    color: var(--fg-1);
    font: inherit;
    font-size: var(--fs-xs);
    text-align: left;
  }
  .opt.hot {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .opt.on {
    color: var(--fg-0);
    font-weight: 600;
  }
  .none {
    padding: 6px;
    color: var(--fg-3);
    font-size: var(--fs-xs);
  }
</style>
