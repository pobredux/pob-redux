<script lang="ts">
  import { parseCalcText } from "$lib/calc-colours";

  let { text, defaultColor = null }: { text: string | null | undefined; defaultColor?: string | null } = $props();
  const spans = $derived(parseCalcText(text));
</script>

{#each spans as span}<span style:color={span.color ?? defaultColor ?? undefined}>{span.text}</span>{/each}
