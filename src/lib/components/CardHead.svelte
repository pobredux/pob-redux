<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "./Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let {
    open,
    title,
    sortable = false,
    onclick,
    onpointerdown,
    onkeydown,
    name,
    extra,
  }: {
    open: boolean;
    title?: string;
    sortable?: boolean;
    onclick: () => void;
    onpointerdown?: (e: PointerEvent) => void;
    onkeydown?: (e: KeyboardEvent) => void;
    name: Snippet;
    extra?: Snippet;
  } = $props();
</script>

<button class="cardhead" aria-expanded={open} {title} {onclick} {onpointerdown} {onkeydown}>
  {#if sortable}<span class="grip" title={m.common_reorder()}><Icon name="dots-six-vertical" size={9} /></span>{/if}
  <span class="cardname">{@render name()}</span>
  {#if extra}<span class="cardsum">{@render extra()}</span>{/if}
  <span class="caret" class:open><Icon name="caret-down" size={11} /></span>
</button>

<style>
  .cardhead {
    position: relative;
    appearance: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 30px;
    padding: 0 7px 0 14px;
    border: 0;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-2);
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: var(--cursor-pointer, pointer);
    user-select: none;
    transition:
      background 120ms,
      color 120ms;
  }
  .cardhead[aria-expanded="false"] {
    border-bottom-color: transparent;
  }
  .cardhead:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .cardhead:focus-visible {
    outline: 1px solid var(--focus);
    outline-offset: -1px;
  }
  :global(.sorting) .cardhead {
    cursor: grabbing;
  }
  .grip {
    position: absolute;
    left: 3px;
    top: 50%;
    translate: 0 -50%;
    display: grid;
    place-items: center;
    color: var(--fg-3);
    opacity: 0;
    cursor: grab;
    transition: opacity 120ms;
  }
  .cardhead:hover .grip,
  :global(.dragging) > .cardhead .grip {
    opacity: 1;
  }
  :global(.sorting) .grip {
    cursor: grabbing;
  }
  .cardname {
    flex: 1 0 auto;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
  }
  .cardsum {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: var(--fs-xs);
    color: var(--fg-0);
    white-space: nowrap;
  }
  .caret {
    flex: none;
    display: grid;
    place-items: center;
    width: 16px;
    height: 16px;
    color: var(--fg-3);
    transition:
      transform 160ms ease,
      color 120ms;
  }
  .caret.open {
    transform: rotate(180deg);
  }
  .cardhead:hover .caret {
    color: var(--fg-1);
  }
</style>
