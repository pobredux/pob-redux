<script lang="ts">
  import type { MenuItem } from "./MoreMenu.svelte";

  let { x, y, label, items, onclose }: { x: number; y: number; label: string; items: MenuItem[]; onclose: () => void } = $props();

  let el = $state<HTMLDivElement | null>(null);
  let width = $state(0);
  let height = $state(0);
  let viewWidth = $state(0);
  let viewHeight = $state(0);
  const left = $derived(Math.max(4, Math.min(x, viewWidth - width - 4)));
  const top = $derived(Math.max(4, Math.min(y, viewHeight - height - 4)));

  $effect(() => {
    (el?.querySelector<HTMLButtonElement>("[role=menuitem]:not(:disabled)") ?? el)?.focus();
  });

  function pick(item: MenuItem) {
    onclose();
    item.onclick();
  }

  function outside(e: PointerEvent) {
    if (el && !el.contains(e.target as Node)) onclose();
  }

  function key(e: KeyboardEvent) {
    const list = [...(el?.querySelectorAll<HTMLButtonElement>("[role=menuitem]:not(:disabled)") ?? [])];
    const i = list.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      list[(i + (e.key === "ArrowDown" ? 1 : -1) + list.length) % list.length]?.focus();
    } else if (e.key === "Tab") onclose();
  }
</script>

<svelte:window bind:innerWidth={viewWidth} bind:innerHeight={viewHeight} onpointerdown={outside} onresize={onclose} onblur={onclose} onscrollcapture={onclose} />

<div
  class="ctx"
  role="menu"
  tabindex="-1"
  aria-label={label}
  bind:this={el}
  bind:offsetWidth={width}
  bind:offsetHeight={height}
  style:left="{left}px"
  style:top="{top}px"
  onkeydown={key}
  oncontextmenu={(e) => e.preventDefault()}
>
  {#each items as item}
    {#if item.separator}<div class="rule" role="separator"></div>{/if}
    <button class="item" class:danger={item.danger} role="menuitem" disabled={item.disabled} title={item.title} onclick={() => pick(item)}>{item.label}</button>
  {/each}
</div>

<style>
  .ctx {
    position: fixed;
    z-index: 60;
    min-width: 160px;
    padding: 4px;
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line-2);
    border-radius: var(--r-1);
    background: var(--bg-2);
    box-shadow: var(--shadow-pop);
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
    cursor: var(--cursor-pointer, pointer);
  }
  .item:hover:not(:disabled),
  .item:focus-visible {
    background: var(--bg-hover);
    color: var(--fg-0);
    outline: none;
  }
  .item:disabled {
    opacity: var(--fade-off);
    cursor: default;
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
