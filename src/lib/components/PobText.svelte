<script lang="ts">
  import { parsePobText } from "$lib/pobtext";
  import { parseCalcText } from "$lib/calc-colours";

  let { text, defaultColor = null, calcs = false }: { text: string | null | undefined; defaultColor?: string | null; calcs?: boolean } = $props();
  const spans = $derived(calcs ? parseCalcText(text) : parsePobText(text));
</script>

{#each spans as s}<span style:color={s.color ?? defaultColor ?? undefined}>{s.text}</span>{/each}
