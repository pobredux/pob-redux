<script lang="ts">
  import { parsePobText, stripPobText, type Span } from "$lib/pobtext";
  import { parseCalcText } from "$lib/calc-colours";
  import { gameText } from "$lib/i18n/game-text.svelte";

  let { text, defaultColor = null, calcs = false }: { text: string | null | undefined; defaultColor?: string | null; calcs?: boolean } = $props();
  // Translate at the display boundary: the engine's labels stay English because
  // some views compare them with literals (CalcsView rows, config placeholders).
  const spans = $derived.by((): Span[] => {
    const translated = gameText(text);
    if (translated === text) return calcs ? parseCalcText(text) : parsePobText(text);
    if (!calcs) return parsePobText(translated);
    // calc-colours.ts tints spans by matching English damage keywords. Those
    // keywords may vanish in a translation, so carry the dominant original
    // colour onto the whole translated line instead of losing the visual cue.
    const colour = parseCalcText(text).find((s) => s.text.trim() && s.color !== null)?.color ?? null;
    if (!colour) return parsePobText(translated);
    return [{ text: stripPobText(translated), color: colour }];
  });
</script>

{#each spans as s}<span style:color={s.color ?? defaultColor ?? undefined}>{s.text}</span>{/each}
