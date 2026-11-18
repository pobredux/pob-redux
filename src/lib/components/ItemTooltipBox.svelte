<script lang="ts">
  import type { Tooltip, TooltipLine } from "$lib/engine.svelte";
  import ItemFrame from "./ItemFrame.svelte";
  import PobText from "./PobText.svelte";

  let { tt, art = true }: { tt: Tooltip; art?: boolean } = $props();

  function lineStyle(l: TooltipLine): string {
    if (l.size >= 20) return "font-size:13px;font-weight:600";
    if (l.size >= 16) return "font-size:12px";
    return "font-size:11px";
  }
</script>

{#if tt.header}
  <div>
    <ItemFrame lines={tt.lines} header={tt.header} runic={tt.runic} uniqueGem={tt.uniqueGem} itemArt={art ? tt.itemArt : undefined} />
  </div>
{:else}
  <div class="plain">
    {#each tt.lines as l}
      {#if l.sep}<div class="sep"></div>
      {:else}<div class="line" class:center={l.center} style={lineStyle(l)}><PobText text={l.text} /></div>{/if}
    {/each}
  </div>
{/if}

<style>
  .plain {
    padding: 10px 12px;
    border: 1px solid var(--line-0);
    border-radius: var(--r-2);
    background: var(--bg-1);
    line-height: 1.45;
  }
  .line {
    color: var(--fg-1);
    white-space: pre-wrap;
  }
  .center {
    text-align: center;
  }
  .sep {
    height: 1px;
    background: var(--line-1);
    margin: 6px 0;
  }
</style>
