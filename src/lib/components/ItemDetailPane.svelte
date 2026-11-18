<script lang="ts">
  import { onDestroy, tick, untrack, type Snippet } from "svelte";
  import { engine, type ItemCustomization, type ItemCustomizationEdit, type Tooltip } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { createScrollHold } from "$lib/scroll-hold";
  import { scrollHint } from "$lib/scroll-hint";
  import ItemCustomizationControls from "./ItemCustomizationControls.svelte";
  import ItemTooltipBox from "./ItemTooltipBox.svelte";

  let {
    itemId,
    generation,
    rev,
    busy = false,
    sync = true,
    transitioning = $bindable(false),
    onedited,
    onmissing,
    children,
  }: {
    itemId: number;
    generation: number;
    /** Reloads the item whenever it changes. */
    rev: number;
    busy?: boolean;
    sync?: boolean;
    transitioning?: boolean;
    onedited?: () => void;
    onmissing?: () => void;
    children?: Snippet;
  } = $props();

  let detail = $state<{ itemId: number; tt: Tooltip; customization: ItemCustomization } | null>(null);
  let pending: { itemId: number; customization: ItemCustomization } | null = null;
  let loading = $state(false);
  let affixPending = $state(false);
  let pane = $state<HTMLDivElement | undefined>();
  let alive = true;
  const scroll = createScrollHold();
  const itemBusy = $derived(busy || loading || affixPending);

  onDestroy(() => {
    alive = false;
  });

  $effect(() => {
    transitioning = loading || affixPending;
  });

  $effect(() => {
    const current = pane;
    return () => scroll.release(current);
  });

  // The floor only has to outlast the edit; left in place, it keeps blank space below content that shrank.
  $effect(() => {
    if (itemBusy) return;
    void tick().then(() => {
      if (!itemBusy) scroll.release();
    });
  });

  $effect(() => {
    const id = itemId;
    rev;
    let active = true;
    untrack(() => {
      loading = true;
      const customization = pending?.itemId === id ? pending.customization : null;
      pending = null;
      Promise.all([engine.itemTooltip({ itemId: id }), customization ?? engine.itemCustomization({ itemId: id, generation })])
        .then(([tt, customization]) => {
          if (!active) return;
          const restore = scroll.keep(detail?.itemId === id ? pane : undefined, () => alive && active);
          detail = { itemId: id, tt, customization };
          restore();
        })
        .catch(() => {
          if (!active) return;
          detail = null;
          onmissing?.();
        })
        .finally(() => {
          if (active) loading = false;
        });
    });
    return () => {
      active = false;
    };
  });

  async function customize(edit: ItemCustomizationEdit) {
    const id = itemId;
    scroll.capture(pane);
    const result = await build.run(async () => {
      const customization = await engine.customizeItem({ itemId: id, generation }, edit);
      pending = { itemId: id, customization };
      return customization;
    }, { sync });
    if (!result) {
      if (pending?.itemId === id) pending = null;
    } else {
      onedited?.();
    }
    return result;
  }

  function setAffixPending(value: boolean) {
    if (value) scroll.capture(pane);
    affixPending = value;
  }
</script>

<div class="pane scroll-hint" {@attach scrollHint} bind:this={pane} onscroll={scroll.follow}>
  <div class="detailcontent">
    {#if detail?.itemId === itemId}
      <ItemTooltipBox tt={detail.tt} />
      <ItemCustomizationControls
        data={detail.customization}
        target={{ itemId, generation }}
        busy={itemBusy}
        onchange={customize}
        onpendingchange={setAffixPending}
      />
      {@render children?.()}
    {/if}
  </div>
</div>

<style>
  .pane {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
  }
  .detailcontent {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 10px 12px;
    box-sizing: border-box;
    flex-shrink: 0;
  }
</style>
