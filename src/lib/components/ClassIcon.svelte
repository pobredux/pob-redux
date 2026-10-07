<script lang="ts">
  import { classIconUrl } from "$lib/item-art";
  import { build } from "$lib/state/build.svelte";
  import type { Game } from "$lib/state/game.svelte";
  import { loadTree } from "$lib/tree/load";
  import ClassArt from "./ClassArt.svelte";

  let {
    game,
    version,
    className,
    ascendancy = null,
    size = 20,
  }: { game: Game; version: string; className: string; ascendancy?: string | null; size?: number } = $props();

  let url = $state<string | null | undefined>(undefined);

  $effect(() => {
    const g = game;
    const cls = className;
    const asc = ascendancy;
    const engineClass = build.classes.find((c) => c.name === cls);
    let live = true;
    loadTree(version)
      .catch(() => null)
      .then((tree) => {
        const treeClass = tree?.model.classes.find((c) => c.name === cls);
        // PoE2 icons use the engine's internal id (Ranger1), PoE1 ones the tree's id (Raider for Warden).
        const ids = (name: string) =>
          [
            engineClass?.ascendancies.find((a) => a.name === name)?.internalId,
            treeClass?.ascendancies.find((a) => a.name === name)?.id,
            name.replace(/\s+/g, ""),
          ].filter((id): id is string => !!id);
        const all = (treeClass?.ascendancies ?? engineClass?.ascendancies ?? []).flatMap((a) => ids(a.name));
        return classIconUrl(g, asc ? ids(asc) : [], all);
      })
      .then((u) => live && (url = u))
      .catch(() => live && (url = null));
    return () => {
      live = false;
    };
  });
</script>

{#if url}
  <img class="icon" src={url} alt="" style:width="{size}px" style:height="{size}px" />
{:else if url === null}
  <ClassArt {version} {className} {ascendancy} {size} />
{:else}
  <span class="icon" style:width="{size}px" style:height="{size}px"></span>
{/if}

<style>
  .icon {
    display: inline-block;
    flex: none;
    border-radius: 50%;
    object-fit: cover;
    background: var(--bg-3);
  }
</style>
