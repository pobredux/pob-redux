<script lang="ts">
  import { segments, type Keyword } from "$lib/tree/keywords";

  let {
    text,
    keywords,
    onpeek,
    onleave,
    onpin,
  }: {
    text: string;
    keywords: Record<string, Keyword>;
    onpeek?: (id: string, el: HTMLElement) => void;
    onleave?: () => void;
    onpin?: (id: string, el: HTMLElement) => void;
  } = $props();

  const parts = $derived(segments(text, keywords));
</script>

{#each parts as part, i (i)}{#if part.id}{@const id = part.id}<button
      type="button"
      class="kw"
      tabindex="-1"
      onpointerenter={(e) => onpeek?.(id, e.currentTarget)}
      onpointerleave={() => onleave?.()}
      onclick={(e) => {
        e.stopPropagation();
        onpin?.(id, e.currentTarget);
      }}>{part.text}</button
    >{:else}{part.text}{/if}{/each}

<style>
  .kw {
    display: inline;
    padding: 0;
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, currentColor 55%, transparent);
    text-underline-offset: 2px;
    cursor: help;
  }
  .kw:hover {
    text-decoration-color: currentColor;
  }
</style>
