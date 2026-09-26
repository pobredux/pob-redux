<script lang="ts">
  import { chat } from "$lib/state/chat.svelte";
  import ComposerMenu from "$lib/components/ComposerMenu.svelte";
  import ProviderIcon from "$lib/components/ProviderIcon.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let open = $state(false);
  let tab = $state(chat.provider);
  let query = $state("");
  let active = $state(0);
  let searchEl = $state<HTMLInputElement | null>(null);

  const provider = $derived(chat.providers.find((p) => p.id === tab));
  const entry = $derived(chat.catalog[tab]);
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = (entry?.models ?? []).filter((x) => !chat.isHidden(tab, x.id) || (chat.provider === tab && chat.model === x.id));
    return q ? list.filter((x) => x.label.toLowerCase().includes(q) || x.id.toLowerCase().includes(q)) : list;
  });
  const current = $derived(chat.models.find((x) => x.id === chat.model));

  function show(id: string) {
    tab = id;
    query = "";
    active = 0;
    const p = chat.providers.find((x) => x.id === id);
    if (p?.ready) void chat.loadCatalog(id);
  }

  async function pick(id: string) {
    open = false;
    await chat.choose(tab, id);
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = shown.length;
      if (n) active = (active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      document.getElementById(`model-opt-${active}`)?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter" && shown[active]) {
      e.preventDefault();
      void pick(shown[active].id);
    }
  }

  function setUp() {
    open = false;
    chat.openSettings();
  }
</script>

<ComposerMenu bind:open wide label={m.chat_model_picker()} onopen={() => show(chat.provider)}>
  {#snippet trigger()}
    <ProviderIcon id={chat.provider} size={14} />
    <span class="name">{current?.label ?? chat.model}</span>
  {/snippet}
  <div class="picker">
    <div class="rail" role="tablist" aria-label={m.chat_providers()}>
      {#each chat.offered as p (p.id)}
        <button
          role="tab"
          aria-selected={p.id === tab}
          class={["tab", { on: p.id === tab, off: !p.ready }]}
          title={p.ready ? p.label : m.chat_provider_not_ready({ name: p.label })}
          onclick={() => {
            show(p.id);
            searchEl?.focus();
          }}
        >
          <ProviderIcon id={p.id} size={16} />
        </button>
      {/each}
    </div>
    <div class="pane">
      <div class="search">
        <Icon name="magnifying-glass" size={12} />
        <input bind:this={searchEl} {@attach (el) => el.focus()} bind:value={query} placeholder={m.chat_search_models({ name: provider?.label ?? "" })} onkeydown={onKey} oninput={() => (active = 0)} />
      </div>
      <div class="list" role="listbox" aria-label={provider?.label}>
        {#if provider && !provider.ready}
          <div class="note">
            <span>{m.chat_provider_not_ready({ name: provider.label })}</span>
            <button class="btn sm" onclick={setUp}>{m.chat_set_up({ name: provider.label })}</button>
          </div>
        {:else if entry?.loading && !entry.models.length}
          <div class="note dim">{m.chat_loading_models()}</div>
        {:else if entry?.error}
          <div class="note"><span class="err">{entry.error}</span></div>
        {:else if !shown.length}
          <div class="note dim">{m.chat_no_models()}</div>
        {:else}
          {#each shown as x, i (x.id)}
            {@const selected = chat.provider === tab && chat.model === x.id}
            <button
              id="model-opt-{i}"
              role="option"
              aria-selected={selected}
              class={["opt", { hot: i === active }]}
              onpointermove={() => (active = i)}
              onclick={() => pick(x.id)}
            >
              <span class="olabel">{x.label}</span>
              {#if x.label.toLowerCase() !== x.id.toLowerCase()}<span class="oid">{x.id}</span>{/if}
              {#if x.fast}<span class="flag" title={m.chat_fast_available()}><Icon name="lightning" size={11} /></span>{/if}
              {#if selected}<span class="check"><Icon name="check" size={12} /></span>{/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
</ComposerMenu>

<style>
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 150px;
  }
  .picker {
    display: flex;
    height: min(340px, 60vh);
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 4px;
    border-right: 1px solid var(--line-1);
    background: var(--bg-1);
    overflow-x: hidden;
    overflow-y: auto;
    flex: none;
  }
  .tab {
    position: relative;
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    padding: 0;
    background: none;
    border: 0;
    border-radius: var(--r-1);
    color: var(--fg-2);
    cursor: pointer;
  }
  .tab:hover {
    color: var(--fg-0);
    background: var(--bg-hover);
  }
  .tab.on {
    color: var(--fg-0);
    background: var(--bg-active);
  }
  .tab.on::after {
    content: "";
    position: absolute;
    left: -4px;
    top: 7px;
    bottom: 7px;
    width: 2px;
    border-radius: 2px;
    background: var(--fg-0);
  }
  .tab.off {
    opacity: 0.45;
  }
  .pane {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    border-bottom: 1px solid var(--line-1);
    color: var(--fg-3);
  }
  .search input {
    flex: 1;
    min-width: 0;
    height: 32px;
    background: none;
    border: 0;
    outline: none;
    color: var(--fg-0);
    font: inherit;
    font-size: var(--fs-sm);
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }
  .opt {
    display: flex;
    align-items: baseline;
    gap: 6px;
    width: 100%;
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
  .opt.hot {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .olabel {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .oid {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .flag,
  .check {
    align-self: center;
    display: grid;
    place-items: center;
    color: var(--fg-3);
    flex: none;
  }
  .check {
    margin-left: auto;
    color: var(--fg-0);
  }
  .note {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 10px;
    font-size: var(--fs-xs);
    line-height: 1.45;
    color: var(--fg-2);
  }
  .err {
    color: var(--bad);
    word-break: break-word;
  }
</style>
