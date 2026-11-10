<script lang="ts">
  import type { Snippet } from "svelte";
  import type { NodeCompare } from "$lib/engine.svelte";
  import type { TNode } from "$lib/tree/model";
  import PobText from "$lib/components/PobText.svelte";
  import { keycaps } from "$lib/keys";
  import { wheelScroll } from "$lib/tooltip-scroll";
  import { m } from "$lib/paraglide/messages";
  import { gameText } from "$lib/i18n/game-text.svelte";

  let {
    node,
    left,
    top,
    fixed = false,
    el = $bindable(null),
    override,
    socketed,
    grant,
    notCalc = new Set(),
    showMastery = false,
    recipe = null,
    blocked = null,
    diff = null,
    foot,
  }: {
    node: TNode;
    left: number;
    top: number;
    /** Positions against the viewport. */
    fixed?: boolean;
    el?: HTMLDivElement | null;
    override?: { name?: string | null; stats?: string[] | null };
    socketed?: { rarity?: string | null; title?: string | null; name: string };
    grant?: { rarity?: string | null; source?: string | null; slot?: string | null };
    notCalc?: ReadonlySet<string>;
    showMastery?: boolean;
    recipe?: { word: string; name: string; url: string | null }[] | null;
    blocked?: string | null;
    diff?: NodeCompare | null;
    /** Left side of the footer; the node id sits right. */
    foot?: Snippet;
  } = $props();
</script>

<div class="tip" class:fixed bind:this={el} use:wheelScroll={[node, diff]} data-scroll-hint={m.tooltip_scroll_hint({ key: keycaps("Mod")[0] })} style:left={`${left}px`} style:top={`${top}px`}>
  <div class="tip-head">
    <span class="tip-name" class:key={node.kind === "keystone"} class:notable={node.kind === "notable"}>{gameText(override?.name ?? node.name)}</span>
    <span class="label">{node.asc ?? node.kind}</span>
  </div>
  {#if socketed}
    <div class="tip-stat">
      <span class="dim">{m.tree_socketed()}</span>
      <span class="rarity" data-rarity={socketed.rarity ?? ""}>{gameText(socketed.title ?? socketed.name)}</span>
    </div>
  {/if}
  {#if grant}
    <div class="tip-stat">
      <span class="dim">{m.tree_granted_by()}</span>
      <span class="rarity" data-rarity={grant.rarity ?? ""}>{grant.source ? gameText(grant.source) : m.tree_granted_unknown()}</span>{#if grant.slot}<span class="dim"> ({grant.slot})</span>{/if}
    </div>
  {/if}
  {#each override?.stats?.length ? override.stats : node.stats as s}
    {@const nc = notCalc.size > 0 && s.split("\n").some((l) => notCalc.has(l))}
    <div class="tip-stat" class:nc>{gameText(s)}{#if nc}<span class="ncnote">{` ${m.not_calculated()}`}</span>{/if}</div>
  {/each}
  {#if node.masteryEffects && showMastery}
    {#each node.masteryEffects as e (e.effect)}
      <div class="tip-stat dim">{e.stats.map((s) => gameText(s)).join(" / ")}</div>
    {/each}
  {/if}
  {#if node.flavour}
    <div class="tip-flav">{gameText(node.flavour)}</div>
  {/if}
  {#if recipe}
    <div class="tip-recipe">
      <span class="dim">{m.tree_anoint()}</span>
      {#each recipe as r, i (i)}
        <span class="recipe-item" title={r.name}>{#if r.url}<img src={r.url} alt="" />{/if}{r.word}</span>
      {/each}
    </div>
  {/if}
  {#if blocked}
    <div class="tip-warn">{blocked}</div>
  {/if}
  {#if diff}
    <div class="tip-diff">
      {#if diff.changes === 0}
        <div class="dim">{m.tree_no_changes()}</div>
      {:else}
        {#each diff.lines as l}
          <div class:head={l.head}><PobText text={l.text} /></div>
        {/each}
      {/if}
    </div>
  {/if}
  <div class="tip-foot num">
    {@render foot?.()}
    <span class="dim">#{node.id}</span>
  </div>
</div>

<style>
  .tip {
    position: absolute;
    width: max-content;
    min-width: min(320px, calc(100% - 16px));
    max-width: min(540px, calc(100% - 16px));
    max-height: calc(100% - 16px);
    overflow: hidden;
    padding: 10px 12px;
    background: color-mix(in srgb, var(--bg-1) 94%, transparent);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-pop);
    pointer-events: none;
    font-size: var(--fs-sm);
  }
  .tip:global(.overflowing)::after {
    content: attr(data-scroll-hint);
    position: sticky;
    bottom: -10px;
    display: block;
    margin: 8px -12px -10px;
    padding: 5px 12px;
    background: var(--bg-2);
    border-top: 1px solid var(--line-1);
    color: var(--fg-1);
    font-size: 11px;
    text-align: center;
  }
  .tip.fixed {
    position: fixed;
    z-index: 50;
    min-width: 320px;
    max-width: min(540px, calc(100vw - 16px));
    max-height: calc(100vh - 16px);
  }
  .tip-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    margin-bottom: 6px;
  }
  .tip-name {
    font-weight: 600;
    color: var(--fg-0);
  }
  .tip-name.key {
    color: var(--c-rare);
  }
  .tip-name.notable {
    color: var(--c-currency);
  }
  .tip-stat {
    color: var(--c-magic);
    line-height: 1.35;
    white-space: pre-line;
  }
  .tip-stat.nc {
    color: var(--bad);
  }
  .ncnote {
    color: var(--fg-3);
  }
  .tip-stat .rarity {
    color: var(--fg-0);
  }
  .tip-stat .rarity[data-rarity="UNIQUE"] {
    color: var(--c-unique);
  }
  .tip-stat .rarity[data-rarity="RARE"] {
    color: var(--c-rare);
  }
  .tip-stat .rarity[data-rarity="MAGIC"] {
    color: var(--c-magic);
  }
  .tip-flav {
    margin-top: 6px;
    color: var(--c-unique);
    font-style: italic;
    font-size: var(--fs-xs);
  }
  .tip-recipe {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 10px;
    margin-top: 6px;
    font-size: var(--fs-xs);
  }
  .recipe-item {
    display: inline-flex;
    align-items: center;
    gap: 3px;
    color: var(--fg-1);
  }
  .recipe-item img {
    width: 20px;
    height: 20px;
    object-fit: contain;
  }
  .tip-warn {
    margin-top: 6px;
    color: var(--warn);
    font-size: var(--fs-xs);
  }
  .tip-diff {
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--line-0);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    line-height: 1.45;
  }
  .tip-diff .head {
    margin-top: 4px;
    color: var(--fg-1);
    font-family: var(--font-ui);
  }
  .tip-diff .head:first-child {
    margin-top: 0;
  }
  .tip-foot {
    display: flex;
    justify-content: space-between;
    gap: 8px;
    margin-top: 8px;
    padding-top: 6px;
    border-top: 1px solid var(--line-0);
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
</style>
