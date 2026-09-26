<script lang="ts">
  import { chat } from "$lib/state/chat.svelte";
  import { m } from "$lib/paraglide/messages";

  const R = 6;
  const C = 2 * Math.PI * R;

  const tokens = (n: number) => (n >= 1e6 ? `${(n / 1e6).toFixed(1)}m` : n >= 1e4 ? `${Math.round(n / 1e3)}k` : n >= 1e3 ? `${(n / 1e3).toFixed(1)}k` : String(n));
  const pct = $derived(chat.contextUse && chat.contextUse.size ? Math.min(100, (chat.contextUse.used / chat.contextUse.size) * 100) : null);
  const processed = $derived(chat.usage.input + chat.usage.output + chat.usage.cacheRead + chat.usage.cacheWrite);
  const title = $derived(
    [
      chat.contextUse && pct !== null ? m.chat_context_used({ percent: Math.round(pct), used: tokens(chat.contextUse.used), size: tokens(chat.contextUse.size) }) : "",
      processed ? m.chat_context_processed({ total: tokens(processed), cached: tokens(chat.usage.cacheRead) }) : "",
    ]
      .filter(Boolean)
      .join(" · "),
  );
</script>

{#if pct !== null}
  <span class={["meter", { full: pct > 90 }]} title={title} role="img" aria-label={title}>
    <svg viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">
      <circle class="track" cx="8" cy="8" r={R} />
      <circle class="fill" cx="8" cy="8" r={R} stroke-dasharray={C} stroke-dashoffset={C * (1 - pct / 100)} />
    </svg>
  </span>
{:else if processed}
  <span class="meter tokens" {title}>{tokens(processed)}</span>
{/if}

<style>
  .meter {
    display: grid;
    place-items: center;
    height: 24px;
    min-width: 24px;
    color: var(--fg-2);
    cursor: default;
  }
  svg {
    transform: rotate(-90deg);
  }
  circle {
    fill: none;
    stroke-width: 2.25;
  }
  .track {
    stroke: var(--line-2);
  }
  .fill {
    stroke: currentColor;
    stroke-linecap: round;
    transition: stroke-dashoffset 400ms ease;
  }
  .meter.full {
    color: var(--warn);
  }
  .tokens {
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    padding: 0 4px;
  }
</style>
