<script lang="ts" module>
  export interface MenuItem {
    label: string;
    onclick: () => void;
    disabled?: boolean;
    danger?: boolean;
    title?: string;
    /** Draw a rule above this item. */
    separator?: boolean;
  }
</script>

<script lang="ts">
  import Icon, { type IconName } from "$lib/components/Icon.svelte";

  let {
    label,
    items,
    icon = "dots-three",
    align = "start",
    disabled = false,
  }: { label: string; items: MenuItem[]; icon?: IconName; align?: "start" | "end"; disabled?: boolean } = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | null>(null);
  let pop = $state<HTMLDivElement | null>(null);
  let up = $state(false);

  $effect(() => {
    if (!pop || !root) return;
    const r = root.getBoundingClientRect();
    const below = window.innerHeight - r.bottom;
    up = pop.offsetHeight + 4 > below && r.top > below;
  });

  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }

  function pick(item: MenuItem) {
    open = false;
    item.onclick();
  }
</script>

<svelte:window onpointerdown={outside} onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class="more" bind:this={root}>
  <button class="btn trigger" class:on={open} aria-haspopup="menu" aria-expanded={open} aria-label={label} title={label} {disabled} onclick={() => (open = !open)}>
    <Icon name={icon} size={14} />
  </button>
  {#if open}
    <div class="pop" class:end={align === "end"} class:up role="menu" aria-label={label} bind:this={pop}>
      {#each items as item}
        {#if item.separator}<div class="rule" role="separator"></div>{/if}
        <button class="item" class:danger={item.danger} role="menuitem" disabled={item.disabled} title={item.title} onclick={() => pick(item)}>{item.label}</button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .more {
    position: relative;
    flex: none;
  }
  .trigger {
    width: 26px;
    padding: 0;
    justify-content: center;
  }
  .pop {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 40;
    min-width: 180px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line-2);
    border-radius: var(--r-1);
    background: var(--bg-2);
    box-shadow: var(--shadow-pop);
  }
  .pop.end {
    left: auto;
    right: 0;
  }
  .pop.up {
    top: auto;
    bottom: calc(100% + 4px);
  }
  .item {
    appearance: none;
    border: 0;
    background: transparent;
    color: var(--fg-1);
    text-align: left;
    padding: 5px 8px;
    border-radius: 3px;
    font: inherit;
    font-size: var(--fs-sm);
    white-space: nowrap;
  }
  .item:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .item:disabled {
    opacity: var(--fade-off);
  }
  .item.danger {
    color: var(--bad);
  }
  .rule {
    height: 1px;
    margin: 4px 2px;
    background: var(--line-1);
  }
</style>
