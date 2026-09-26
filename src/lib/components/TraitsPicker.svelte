<script lang="ts">
  import { chat } from "$lib/state/chat.svelte";
  import ComposerMenu from "$lib/components/ComposerMenu.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let open = $state(false);

  const names: Record<string, () => string> = {
    none: m.effort_none,
    minimal: m.effort_minimal,
    low: m.effort_low,
    medium: m.effort_medium,
    high: m.effort_high,
    xhigh: m.effort_xhigh,
    max: m.effort_max,
    ultra: m.effort_ultra,
  };
  const name = (e: string) => names[e]?.() ?? e[0].toUpperCase() + e.slice(1);
  const fastOn = $derived(chat.fast && chat.supportsFast);
</script>

<ComposerMenu bind:open label={m.chat_effort()}>
  {#snippet trigger()}
    {#if fastOn}<span class="zap"><Icon name="lightning-fill" size={12} /></span>{/if}
    <span>{chat.effortLevel ? name(chat.effortLevel) : fastOn ? m.chat_speed_fast() : m.chat_speed_standard()}</span>
  {/snippet}
  {#if chat.supportsEffort}
    <div class="head">{m.chat_reasoning()}</div>
    {#each chat.efforts as e (e)}
      <button class="item" role="menuitemradio" aria-checked={chat.effortLevel === e} onclick={() => { chat.setEffort(e); open = false; }}>
        <span>{name(e)}</span>
        {#if chat.effortLevel === e}<span class="check"><Icon name="check" size={12} /></span>{/if}
      </button>
    {/each}
  {/if}
  {#if chat.supportsFast}
    <div class="head">{m.chat_speed()}</div>
    {#each [false, true] as on (on)}
      <button class="item" role="menuitemradio" aria-checked={fastOn === on} onclick={() => { chat.setFast(on); open = false; }}>
        <span class="lead"><Icon name={on ? "lightning" : "brain"} size={12} /></span>
        <span class="text">
          <span>{on ? m.chat_speed_fast() : m.chat_speed_standard()}</span>
          <span class="desc">{on ? m.chat_speed_fast_desc() : m.chat_speed_standard_desc()}</span>
        </span>
        {#if fastOn === on}<span class="check"><Icon name="check" size={12} /></span>{/if}
      </button>
    {/each}
  {/if}
</ComposerMenu>

<style>
  .zap {
    display: grid;
    place-items: center;
    color: var(--fg-0);
  }
  .head {
    padding: 6px 8px 3px;
    font-size: var(--fs-2xs);
    color: var(--fg-3);
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    background: none;
    border: 0;
    border-radius: var(--r-1);
    color: var(--fg-1);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
    cursor: pointer;
  }
  .item:hover {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .lead {
    display: grid;
    place-items: center;
    color: var(--fg-3);
    align-self: flex-start;
    margin-top: 3px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    flex: 1;
    min-width: 0;
  }
  .desc {
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    line-height: 1.35;
  }
  .check {
    margin-left: auto;
    display: grid;
    place-items: center;
    color: var(--fg-0);
  }
</style>
