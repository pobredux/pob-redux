<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { isNumeric, parse, shortUrl, tradeLeague, type Span } from "$lib/ai/markdown";
  import type { Tooltip } from "$lib/engine.svelte";
  import { m } from "$lib/paraglide/messages";
  import { gems } from "$lib/state/gems.svelte";
  import { tradeWindow } from "$lib/state/trade.svelte";
  import PobTooltip from "./PobTooltip.svelte";

  let { text }: { text: string } = $props();
  // Re-parses when the gem index arrives, so names light up in replies that
  // were rendered before it loaded.
  const blocks = $derived(parse(text, gems.pattern ? (t) => gems.scan(t) : undefined));

  let tip = $state<{ tt: Tooltip; x: number; y: number } | null>(null);
  let tipTimer = 0;

  function showTip(e: MouseEvent, gemId: string) {
    clearTimeout(tipTimer);
    const x = Math.max(8, Math.min(e.clientX + 16, window.innerWidth - 560));
    const y = Math.min(e.clientY + 12, window.innerHeight - 420);
    tipTimer = window.setTimeout(async () => {
      try {
        tip = { tt: await gems.tooltip(gemId), x, y };
      } catch {
        tip = null;
      }
    }, 120);
  }
  function hideTip() {
    clearTimeout(tipTimer);
    tip = null;
  }

  function linkText(text: string, href: string) {
    if (text) return text;
    const league = tradeLeague(href);
    return league ? m.chat_trade_link({ league }) : shortUrl(href);
  }

  // The webview would navigate away from the app on a plain link click.
  function follow(e: MouseEvent, href: string) {
    e.preventDefault();
    if (tradeLeague(href)) void tradeWindow.open(href);
    else void openUrl(href);
  }
</script>

{#snippet spans(list: Span[])}
  {#each list as s, i (i)}
    {#if s.kind === "gem"}
      <span
        class="gem {s.gem.kind}"
        role="note"
        onmouseenter={(e) => showTip(e, s.gem.gemId)}
        onmouseleave={hideTip}>{s.text}</span>
    {:else if s.kind === "link"}
      <a href={s.href} title={s.href} onclick={(e) => follow(e, s.href)} onauxclick={(e) => e.preventDefault()}>{linkText(s.text, s.href)}</a>
    {:else if s.kind === "bold"}<b>{s.text}</b>{:else if s.kind === "code"}<code>{s.text}</code>{:else}{s.text}{/if}
  {/each}
{/snippet}

<div class="md">
  {#each blocks as b, i (i)}
    {#if b.kind === "p"}
      <p>{@render spans(b.spans)}</p>
    {:else if b.kind === "h"}
      <p class="h">{@render spans(b.spans)}</p>
    {:else if b.kind === "list"}
      {#if b.ordered}
        <ol>{#each b.items as item, j (j)}<li>{@render spans(item)}</li>{/each}</ol>
      {:else}
        <ul>{#each b.items as item, j (j)}<li>{@render spans(item)}</li>{/each}</ul>
      {/if}
    {:else if b.kind === "table"}
      <div class="twrap">
        <table>
          <thead>
            <tr>{#each b.head as c, j (j)}<th class:num={isNumeric(c)}>{@render spans(c)}</th>{/each}</tr>
          </thead>
          <tbody>
            {#each b.rows as row, r (r)}
              <tr>{#each row as c, j (j)}<td class:num={isNumeric(c)}>{@render spans(c)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      </div>
    {:else if b.kind === "pre"}
      <pre>{b.text}</pre>
    {/if}
  {/each}
</div>

{#if tip}
  <PobTooltip lines={tip.tt.lines} header={tip.tt.header} runic={tip.tt.runic} uniqueGem={tip.tt.uniqueGem} x={tip.x} y={tip.y} />
{/if}

<style>
  .md {
    display: flex;
    flex-direction: column;
    gap: 8px;
    word-break: break-word;
  }
  p,
  ul,
  ol,
  pre {
    margin: 0;
  }
  .h {
    font-weight: 600;
    color: var(--fg-0);
  }
  ul,
  ol {
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  code,
  pre {
    font-family: var(--font-mono);
    font-size: 0.92em;
  }
  code {
    padding: 0 3px;
    background: var(--bg-3);
    border-radius: 3px;
  }
  pre {
    padding: 6px 8px;
    background: var(--bg-3);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow-x: auto;
    white-space: pre;
  }
  /* Gem names carry PoB's own meanings: gem teal for a skill, spirit gold for
     what is switched on once, magic blue for a support. Hover for the card. */
  .gem {
    cursor: help;
    text-decoration: underline dotted;
    text-underline-offset: 3px;
    text-decoration-color: color-mix(in srgb, currentColor 45%, transparent);
  }
  .gem.skill {
    color: var(--c-gem);
  }
  .gem.spirit {
    color: var(--c-spirit);
  }
  .gem.support {
    color: var(--c-magic);
  }
  a {
    color: var(--focus);
    text-decoration: underline;
    text-underline-offset: 3px;
    text-decoration-color: color-mix(in oklab, currentColor 45%, transparent);
  }
  a:hover {
    text-decoration-color: currentColor;
  }
  .twrap {
    overflow-x: auto;
  }
  table {
    border-collapse: collapse;
    font-size: var(--fs-sm);
    min-width: 60%;
  }
  th,
  td {
    text-align: left;
    padding: 4px 10px 4px 0;
    border-bottom: 1px solid var(--line-1);
    vertical-align: top;
  }
  th {
    font-weight: 500;
    color: var(--fg-2);
  }
  tr:last-child td {
    border-bottom: 0;
  }
  .num {
    font-family: var(--font-mono);
    text-align: right;
    padding-right: 0;
    padding-left: 10px;
    white-space: nowrap;
  }
</style>
