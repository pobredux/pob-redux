<script lang="ts">
  import { chat } from "$lib/state/chat.svelte";
  import ComposerMenu from "$lib/components/ComposerMenu.svelte";
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let open = $state(false);

  const options = $derived<{ ask: boolean; icon: IconName; label: string; desc: string }[]>([
    { ask: false, icon: "sparkle", label: m.chat_changes_auto(), desc: m.chat_changes_auto_desc() },
    { ask: true, icon: "lock-simple", label: m.chat_changes_supervised(), desc: m.chat_changes_supervised_desc() },
  ]);
  const now = $derived(options.find((o) => o.ask === chat.askFirst) ?? options[0]);
</script>

<ComposerMenu bind:open label={now.desc}>
  {#snippet trigger()}
    <Icon name={now.icon} size={12} />
    <span class="lbl">{now.label}</span>
  {/snippet}
  {#each options as o (o.ask)}
    <button class="item" role="menuitemradio" aria-checked={chat.askFirst === o.ask} onclick={() => { chat.setAskFirst(o.ask); open = false; }}>
      <span class="lead"><Icon name={o.icon} size={13} /></span>
      <span class="text">
        <span>{o.label}</span>
        <span class="desc">{o.desc}</span>
      </span>
      {#if chat.askFirst === o.ask}<span class="check"><Icon name="check" size={12} /></span>{/if}
    </button>
  {/each}
</ComposerMenu>

<style>
  .item {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    width: 100%;
    max-width: 260px;
    padding: 6px 8px;
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
    margin-top: 2px;
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
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
    margin-top: 2px;
  }
  @container composer (max-width: 360px) {
    .lbl {
      display: none;
    }
  }
</style>
