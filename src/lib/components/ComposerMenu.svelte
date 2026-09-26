<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "$lib/components/Icon.svelte";

  let {
    open = $bindable(false),
    label,
    wide = false,
    disabled = false,
    onopen,
    trigger,
    children,
  }: {
    open?: boolean;
    label: string;
    /** Span the whole composer instead of hanging off the button. */
    wide?: boolean;
    disabled?: boolean;
    onopen?: () => void;
    trigger: Snippet;
    children: Snippet;
  } = $props();

  let root = $state<HTMLDivElement | null>(null);
  let alignEnd = $state(false);

  function toggle() {
    if (!open && root && !wide) {
      const box = root.closest(".composer")?.getBoundingClientRect();
      const at = root.getBoundingClientRect();
      alignEnd = !!box && at.left + 250 > box.right;
    }
    open = !open;
    if (open) onopen?.();
  }

  function outside(e: PointerEvent) {
    if (open && root && !root.contains(e.target as Node)) open = false;
  }
</script>

<svelte:window onpointerdown={outside} onkeydown={(e) => open && e.key === "Escape" && (open = false)} />

<div class={["menu", { wide }]} bind:this={root}>
  <button class={["ctl", { on: open }]} aria-haspopup="true" aria-expanded={open} title={label} {disabled} onclick={toggle}>
    {@render trigger()}
    <span class="caret"><Icon name="caret-down" size={10} /></span>
  </button>
  {#if open}
    <div class={["pop", { end: alignEnd, wide }]} role="dialog" aria-label={label}>
      {@render children()}
    </div>
  {/if}
</div>

<style>
  .menu {
    position: relative;
    min-width: 0;
  }
  .menu.wide {
    position: static;
  }
  .ctl {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    max-width: 100%;
    padding: 0 6px;
    background: none;
    border: 0;
    border-radius: var(--r-1);
    color: var(--fg-2);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
    white-space: nowrap;
  }
  .ctl:hover:not(:disabled),
  .ctl.on {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .ctl:disabled {
    color: var(--fg-4);
    cursor: default;
  }
  .caret {
    display: grid;
    place-items: center;
    color: var(--fg-3);
    flex: none;
  }
  .pop {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    z-index: 20;
    min-width: 220px;
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    padding: 4px;
  }
  .pop.end {
    left: auto;
    right: 0;
  }
  .pop.wide {
    left: 0;
    right: 0;
    min-width: 0;
    padding: 0;
    overflow: hidden;
  }
</style>
