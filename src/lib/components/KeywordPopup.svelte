<script lang="ts">
  import type { Keyword } from "$lib/tree/keywords";
  import KeywordText from "./KeywordText.svelte";
  import Icon from "./Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let {
    keyword,
    keywords,
    x,
    y,
    area,
    up = false,
    pinned = false,
    onpeek,
    onleave,
    onpin,
    onclose,
    onmove,
  }: {
    keyword: Keyword;
    keywords: Record<string, Keyword>;
    x: number;
    y: number;
    /** The size of the view the popup is placed in. */
    area: { w: number; h: number };
    /** `y` is the popup's bottom edge. */
    up?: boolean;
    pinned?: boolean;
    onpeek?: (id: string, el: HTMLElement) => void;
    onleave?: () => void;
    onpin?: (id: string, el: HTMLElement) => void;
    onclose?: () => void;
    onmove?: (x: number, y: number) => void;
  } = $props();

  let width = $state(0);
  let height = $state(0);
  const top = $derived(up ? Math.max(8, y - height) : Math.max(8, Math.min(y, area.h - 8 - height)));
  const movable = $derived(pinned && onmove != null);
  let drag = $state<{ px: number; py: number; x: number; y: number } | null>(null);

  function grab(e: PointerEvent) {
    if (!movable || e.button !== 0 || (e.target as HTMLElement).closest(".close")) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { px: e.clientX, py: e.clientY, x, y: top };
  }

  function move(e: PointerEvent) {
    if (!drag) return;
    const nx = Math.max(8, Math.min(drag.x + e.clientX - drag.px, area.w - 8 - width));
    const ny = Math.max(8, Math.min(drag.y + e.clientY - drag.py, area.h - 8 - height));
    onmove?.(nx, ny);
  }
</script>

<div class="kwpop" class:pinned role="note" bind:offsetWidth={width} bind:offsetHeight={height} style:left={`${x}px`} style:top={`${top}px`}>
  <div
    class="head"
    class:movable
    class:dragging={drag != null}
    role="presentation"
    onpointerdown={grab}
    onpointermove={move}
    onpointerup={() => (drag = null)}
    onpointercancel={() => (drag = null)}
  >
    <span class="term">{keyword.term}</span>
    {#if pinned}
      <button type="button" class="close" aria-label={m.common_close()} title={m.common_close()} onclick={onclose}>
        <Icon name="x" size={12} />
      </button>
    {/if}
  </div>
  <div class="body"><KeywordText text={keyword.definition} {keywords} {onpeek} {onleave} {onpin} /></div>
</div>

<style>
  .kwpop {
    position: absolute;
    z-index: 4;
    width: max-content;
    max-width: min(380px, calc(100% - 16px));
    padding: 8px 10px 10px;
    background: var(--bg-1);
    border: 1px solid var(--line-2);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    font-size: var(--fs-sm);
  }
  .kwpop.pinned {
    z-index: 3;
    border-color: var(--line-1);
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }
  .head.movable {
    cursor: grab;
    touch-action: none;
    user-select: none;
  }
  .head.dragging {
    cursor: grabbing;
  }
  .term {
    font-weight: 600;
    color: var(--fg-0);
  }
  .close {
    display: inline-flex;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--fg-2);
    cursor: var(--cursor-pointer, pointer);
  }
  .close:hover {
    color: var(--fg-0);
  }
  .body {
    color: var(--fg-1);
    line-height: 1.4;
    white-space: pre-line;
  }
</style>
