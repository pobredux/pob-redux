<script lang="ts">
  import TreeView from "$lib/views/TreeView.svelte";
  import OverviewView from "$lib/views/OverviewView.svelte";
  import SkillsView from "$lib/views/SkillsView.svelte";
  import ItemsView from "$lib/views/ItemsView.svelte";
  import CalcsView from "$lib/views/CalcsView.svelte";
  import ConfigView from "$lib/views/ConfigView.svelte";
  import NotesView from "$lib/views/NotesView.svelte";
  import PartyView from "$lib/views/PartyView.svelte";
  import OptimiseView from "$lib/views/OptimiseView.svelte";
  import CompareView from "$lib/views/CompareView.svelte";
  import ImportView from "$lib/views/ImportView.svelte";
  import type { AppPaths } from "$lib/engine.svelte";
  import { build, type ViewId } from "$lib/state/build.svelte";

  let { view, paths = null }: { view: ViewId; paths?: AppPaths | null } = $props();
</script>

{#if view === "overview"}
  <OverviewView />
{:else if view === "tree"}
  <TreeView />
{:else if view === "skills"}
  <SkillsView />
{:else if view === "items"}
  {#key build.info?.generation}
    <ItemsView />
  {/key}
{:else if view === "calcs"}
  <CalcsView />
{:else if view === "config"}
  <ConfigView />
{:else if view === "notes"}
  <NotesView />
{:else if view === "party"}
  <PartyView />
{:else if view === "optimise"}
  <OptimiseView />
{:else if view === "compare"}
  <CompareView />
{:else}
  <ImportView {paths} />
{/if}
