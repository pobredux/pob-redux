<script lang="ts">
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { listen } from "@tauri-apps/api/event";
  import {
    cancelSignIn,
    installAgent,
    openTerminal,
    setAgentPath,
    setBase,
    signInAgent,
    signOutAgent,
    uninstallAgent,
    type ProviderStatus,
  } from "$lib/ai/providers";
  import { chat } from "$lib/state/chat.svelte";
  import ProviderIcon from "$lib/components/ProviderIcon.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { m } from "$lib/paraglide/messages";

  let selected = $state(chat.provider);
  const p = $derived(chat.providers.find((x) => x.id === selected) ?? chat.providers[0]);

  let error = $state<string | null>(null);
  let checking = $state(false);
  let notes = $state<Record<string, string>>({});
  let installing = $state<Record<string, string>>({});
  let signingIn = $state<string | null>(null);
  let signInLink = $state<string | null>(null);
  let location = $state("");
  let filter = $state("");
  let copied = $state("");
  let now = $state(Date.now());

  const version = (v: string | null | undefined) => v?.match(/\d+(?:\.\d+)+/)?.[0] ?? "";

  function status(x: ProviderStatus): { text: string; tone: "ok" | "warn" | "bad" } {
    const a = x.agent;
    if (!a) return { text: m.assistant_status_local(), tone: "ok" };
    if (!a.installed) return { text: m.assistant_status_missing(), tone: "bad" };
    if (a.error) return { text: m.assistant_status_broken(), tone: "bad" };
    if (a.signed_in === false) return { text: m.assistant_status_signed_out(), tone: "warn" };
    if (a.signed_in) return { text: a.account ? m.assistant_status_signed_in({ account: a.account }) : m.assistant_status_signed_in_plain(), tone: "ok" };
    return { text: m.assistant_status_installed(), tone: "ok" };
  }

  const checked = $derived.by(() => {
    if (!chat.checkedAt) return "";
    const s = Math.round((now - chat.checkedAt) / 1000);
    return s < 30 ? m.assistant_checked_now() : s < 3600 ? m.assistant_checked_minutes({ minutes: Math.max(1, Math.round(s / 60)) }) : m.assistant_checked_hours({ hours: Math.round(s / 3600) });
  });

  const models = $derived(chat.catalog[p?.id ?? ""]);
  const listed = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const all = models?.models ?? [];
    return q ? all.filter((x) => x.label.toLowerCase().includes(q) || x.id.toLowerCase().includes(q)) : all;
  });
  const shownCount = $derived((models?.models ?? []).filter((x) => !chat.isHidden(p?.id ?? "", x.id)).length);

  function select(id: string) {
    selected = id;
    filter = "";
    location = "";
    error = null;
    const x = chat.providers.find((y) => y.id === id);
    if (x?.ready) void chat.loadCatalog(id);
  }

  async function run(fn: () => Promise<unknown>) {
    error = null;
    try {
      await fn();
    } catch (e) {
      error = String(e);
    }
  }

  async function recheckAll() {
    checking = true;
    await run(async () => {
      await chat.refreshProviders(true);
      await chat.refreshModels();
      if (p?.ready) void chat.loadCatalog(p.id);
    });
    checking = false;
  }

  /** After a terminal opens, look again every few seconds until the provider changes or five minutes pass. */
  function watch(id: string) {
    const before = JSON.stringify(chat.providers.find((x) => x.id === id)?.agent);
    const until = Date.now() + 5 * 60_000;
    const tick = async () => {
      await chat.refreshProviders(true, id);
      const after = chat.providers.find((x) => x.id === id);
      if (JSON.stringify(after?.agent) !== before) {
        delete notes[id];
        if (after?.ready) void chat.loadCatalog(id, true);
        if (id === chat.provider) await chat.refreshModels();
        return;
      }
      if (Date.now() < until) setTimeout(tick, 4000);
    };
    setTimeout(tick, 4000);
  }

  function terminal(x: ProviderStatus, action: "install" | "login") {
    void run(async () => {
      const cmd = await openTerminal(x.id, action);
      notes[x.id] = action === "install" ? m.assistant_terminal_install({ command: cmd }) : m.assistant_terminal_login({ command: cmd });
      watch(x.id);
    });
  }

  function download(x: ProviderStatus) {
    installing[x.id] = m.assistant_downloading({ percent: 0 });
    void run(async () => {
      try {
        await installAgent(x.id, (e) => {
          installing[x.id] = e.kind === "unpack" ? m.assistant_unpacking() : m.assistant_downloading({ percent: Math.floor((e.done * 100) / e.total) });
        });
        await chat.refreshProviders();
      } finally {
        delete installing[x.id];
      }
    });
  }

  async function signIn(x: ProviderStatus) {
    signingIn = x.id;
    signInLink = null;
    const unlisten = await listen<{ provider: string; url: string }>("agent:sign-in", (e) => {
      if (e.payload.provider === x.id) signInLink = e.payload.url;
    });
    await run(async () => {
      try {
        await signInAgent(x.id);
        await chat.refreshProviders();
        void chat.loadCatalog(x.id, true);
      } catch (e) {
        if (signingIn) throw e;
      } finally {
        unlisten();
        signingIn = null;
        signInLink = null;
      }
    });
  }

  function stopSignIn() {
    signingIn = null;
    void cancelSignIn();
  }

  function saveLocation(x: ProviderStatus) {
    void run(async () => {
      if (x.agent) await setAgentPath(x.id, location);
      else await setBase(x.id, location);
      await chat.refreshProviders(!!x.agent, x.agent ? x.id : undefined);
      if (x.id === chat.provider) await chat.refreshModels();
      location = "";
    });
  }

  async function copy(key: string, text: string) {
    await writeText(text).catch(() => {});
    copied = key;
    setTimeout(() => (copied = ""), 1400);
  }

  $effect(() => {
    if (p?.ready && !chat.catalog[p.id]) void chat.loadCatalog(p.id);
  });
</script>

<div class="bar" {@attach () => {
  const id = setInterval(() => (now = Date.now()), 30_000);
  return () => clearInterval(id);
}}>
  <button class="btn sm ghost" disabled={checking} onclick={recheckAll} title={m.provider_check_again()}>
    <span class={["spin", { on: checking }]}><Icon name="arrows-clockwise" size={11} /></span>
    {checking ? m.assistant_checking() : checked || m.provider_check_again()}
  </button>
</div>

<div class="wrap">
<div class="panes">
  <div class="list" role="listbox" aria-label={m.assistant_providers()}>
    {#each chat.providers as x (x.id)}
      {@const st = status(x)}
      {@const on = !chat.disabled.includes(x.id)}
      <div class={["item", { sel: x.id === p?.id, off: !on }]}>
        <button class="pick" role="option" aria-selected={x.id === p?.id} onclick={() => select(x.id)}>
          <span class="icon"><ProviderIcon id={x.id} size={16} /></span>
          <span class="meta">
            <span class="top">
              <span class="name">{x.label}</span>
              {#if version(x.agent?.version)}<span class="ver">v{version(x.agent?.version)}</span>{/if}
            </span>
            <span class="status"><span class="dot {st.tone}"></span>{st.text}</span>
          </span>
        </button>
        <input class="switch" type="checkbox" role="switch" checked={on} aria-label={m.assistant_offer({ name: x.label })} title={m.assistant_offer({ name: x.label })} onchange={(e) => chat.setEnabled(x.id, (e.target as HTMLInputElement).checked)} />
      </div>
    {/each}
  </div>

  {#if p}
    {@const st = status(p)}
    <div class="detail">
      <div class="dhead">
        <ProviderIcon id={p.id} size={18} />
        <span class="dname">{p.label}</span>
        {#if version(p.agent?.version)}<span class="ver">v{version(p.agent?.version)}</span>{/if}
        <span class="grow"></span>
        {#if p.id === chat.provider}
          <span class="badge"><span class="dot {p.ready ? 'ok' : 'warn'}"></span>{m.assistant_in_use()}</span>
        {:else}
          <button class="btn sm" disabled={!p.ready || chat.disabled.includes(p.id)} onclick={() => chat.setProvider(p.id)}>{m.provider_use()}</button>
        {/if}
      </div>

      <h3 class="sub">{m.assistant_setup()}</h3>
      <div class="rows">
        {#if p.agent}
          <div class="opt">
            <span>
              {m.assistant_program()}
              <span class="hint mono">{p.agent.installed ? p.agent.path : m.assistant_program_missing()}</span>
            </span>
            <div class="acts">
              {#if p.download}
                {#if installing[p.id]}
                  <span class="hint mono">{installing[p.id]}</span>
                {:else if p.agent.installed}
                  <button class="btn sm ghost" title={m.assistant_remove()} aria-label={m.assistant_remove()} onclick={() => run(async () => { await uninstallAgent(p.id); await chat.refreshProviders(); })}><Icon name="trash" size={12} /></button>
                  <button class="btn sm ghost" onclick={() => download(p)}>{m.assistant_reinstall()}</button>
                {:else}
                  <button class="btn sm" onclick={() => download(p)}>{m.assistant_download({ size: Math.round(p.download / 1e6) })}</button>
                {/if}
              {:else if p.install_command}
                <button class={["btn", "sm", { ghost: p.agent.installed }]} onclick={() => terminal(p, "install")}>{p.agent.installed ? m.assistant_reinstall() : m.assistant_install()}</button>
              {/if}
            </div>
          </div>
          {#if !p.agent.installed && (p.install_command || p.download)}
            <div class="opt col">
              {#if p.download}
                <span class="hint">{m.assistant_download_note()}</span>
              {:else if p.install_command}
                <span class="hint">{m.assistant_install_hint()}</span>
                <div class="cmdrow">
                  <code class="cmd">{p.install_command}</code>
                  <button class="btn sm ghost" onclick={() => copy(`i-${p.id}`, p.install_command!)}>{copied === `i-${p.id}` ? m.common_copied() : m.common_copy()}</button>
                </div>
              {/if}
              {#if p.install}<button class="link" onclick={() => openUrl(p.install!)}>{m.provider_install_link({ name: p.label })}</button>{/if}
            </div>
          {:else if !p.agent.installed}
            <div class="opt col"><span class="hint">{m.assistant_no_build({ name: p.label })}</span></div>
          {/if}
          {#if p.agent.installed}
            <div class="opt">
              <span>
                {m.assistant_account()}
                <span class={["hint", { warn: st.tone !== "ok" }]}>{p.agent.error ?? st.text}</span>
              </span>
              <div class="acts">
                {#if p.download}
                  {#if signingIn === p.id}
                    {#if signInLink}<button class="link" onclick={() => openUrl(signInLink!)}>{m.assistant_open_sign_in()}</button>{/if}
                    <button class="btn sm ghost" onclick={stopSignIn}>{m.common_cancel()}</button>
                  {:else if p.agent.signed_in}
                    <button class="btn sm ghost" onclick={() => run(async () => { await signOutAgent(p.id); await chat.refreshProviders(); })}>{m.assistant_sign_out()}</button>
                  {:else}
                    <button class="btn sm" disabled={!!signingIn} onclick={() => signIn(p)}>{m.assistant_sign_in_google()}</button>
                  {/if}
                {:else if p.login}
                  <button class={["btn", "sm", { ghost: p.agent.signed_in !== false }]} onclick={() => terminal(p, "login")}>{p.agent.signed_in ? m.assistant_sign_in_again() : m.assistant_sign_in()}</button>
                {/if}
              </div>
            </div>
            {#if signingIn === p.id}
              <div class="opt col"><span class="hint">{m.assistant_signing_in()}</span></div>
            {/if}
          {/if}
          {#if notes[p.id]}
            <div class="opt col"><span class="hint">{notes[p.id]}</span></div>
          {/if}
        {/if}
        <div class="opt">
          <span>
            {p.agent ? m.assistant_path() : m.assistant_address()}
            <span class="hint">{p.agent ? m.provider_path_hint({ name: p.id }) : m.provider_base_hint({ base: p.default_base ?? "" })}</span>
          </span>
          <div class="acts">
            <input class="input mono addr" bind:value={location} placeholder={p.agent ? m.provider_path_placeholder() : (p.base_url ?? "")} />
            <button class="btn sm ghost" disabled={!location.trim() && !!p.agent} onclick={() => saveLocation(p)}>{m.common_save()}</button>
          </div>
        </div>
      </div>

      <h3 class="sub">
        {m.assistant_models()}
        {#if models?.models.length}<span class="count">{m.assistant_models_count({ shown: shownCount, total: models.models.length })}</span>{/if}
      </h3>
      <div class="rows">
        {#if !p.ready}
          <div class="opt col"><span class="hint">{m.assistant_models_need_setup({ name: p.label })}</span></div>
        {:else if models?.loading && !models.models.length}
          <div class="opt col"><span class="hint">{m.chat_loading_models()}</span></div>
        {:else if models?.error}
          <div class="opt col"><span class="hint warn mono">{models.error}</span></div>
        {:else if models?.models.length}
          <div class="opt filter">
            <input class="input" bind:value={filter} placeholder={m.assistant_filter_models()} />
            <div class="acts">
              <button class="btn sm ghost" onclick={() => chat.setHidden(p.id, listed.map((x) => x.id), false)}>{m.assistant_show_all()}</button>
              <button class="btn sm ghost" onclick={() => chat.setHidden(p.id, listed.filter((x) => !(p.id === chat.provider && x.id === chat.model)).map((x) => x.id), true)}>{m.assistant_hide_all()}</button>
            </div>
          </div>
          <div class="mlist">
            {#each listed as x (x.id)}
              {@const inUse = p.id === chat.provider && x.id === chat.model}
              <label class="mrow">
                <span class="mname">{x.label}</span>
                {#if x.label.toLowerCase() !== x.id.toLowerCase()}<span class="mid">{x.id}</span>{/if}
                <span class="grow"></span>
                <span class="tags">
                  {[x.fast ? m.assistant_tag_fast() : "", x.efforts.length ? m.assistant_tag_reasoning() : ""].filter(Boolean).join(" · ")}
                </span>
                <input
                  class="switch"
                  type="checkbox"
                  role="switch"
                  checked={!chat.isHidden(p.id, x.id)}
                  disabled={inUse}
                  title={inUse ? m.assistant_model_in_use() : m.assistant_model_show()}
                  onchange={(e) => chat.setHidden(p.id, [x.id], !(e.target as HTMLInputElement).checked)}
                />
              </label>
            {/each}
          </div>
        {:else}
          <div class="opt col"><span class="hint">{m.chat_no_models()}</span></div>
        {/if}
      </div>
    </div>
  {/if}
</div>

</div>

{#if error}<div class="err mono">{error}</div>{/if}

<style>
  .bar {
    display: flex;
    justify-content: flex-end;
    margin: -8px 0 8px;
  }
  .spin {
    display: inline-grid;
    place-items: center;
  }
  .spin.on {
    animation: turn 900ms linear infinite;
  }
  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
  .wrap {
    container: panes / inline-size;
  }
  .panes {
    display: grid;
    grid-template-columns: minmax(220px, 260px) 1fr;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    background: var(--bg-1);
    min-height: 420px;
  }
  .list {
    border-right: 1px solid var(--line-1);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-right: 8px;
    border-radius: var(--r-1);
  }
  .item:hover {
    background: var(--bg-hover);
  }
  .item.sel {
    background: var(--bg-active);
  }
  .item.off .pick {
    opacity: 0.55;
  }
  .pick {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    flex: 1;
    min-width: 0;
    padding: 9px 8px;
    background: none;
    border: 0;
    color: var(--fg-1);
    font: inherit;
    text-align: left;
    cursor: pointer;
  }
  .icon {
    display: grid;
    place-items: center;
    margin-top: 1px;
    color: var(--fg-0);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }
  .top {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
  }
  .name {
    color: var(--fg-0);
    font-size: var(--fs-md);
    font-weight: 600;
  }
  .ver {
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
    background: var(--fg-3);
  }
  .dot.ok {
    background: var(--ok);
  }
  .dot.warn {
    background: var(--warn);
  }
  .dot.bad {
    background: var(--bad);
  }
  .detail {
    padding: 14px 16px 16px;
    min-width: 0;
  }
  .dhead {
    display: flex;
    align-items: center;
    gap: 8px;
    color: var(--fg-0);
    padding-bottom: 12px;
    border-bottom: 1px solid var(--line-1);
  }
  .dname {
    font-size: var(--fs-lg);
    font-weight: 600;
  }
  .grow {
    flex: 1;
  }
  .badge {
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 9px;
    border: 1px solid var(--line-1);
    border-radius: 999px;
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
  .sub {
    display: flex;
    align-items: baseline;
    gap: 8px;
    margin: 16px 2px 8px;
    font-size: var(--fs-sm);
    font-weight: 500;
    color: var(--fg-1);
  }
  .count {
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    font-weight: 400;
  }
  .rows {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    background: var(--bg-0);
  }
  .opt {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    min-height: 46px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
    color: var(--fg-1);
  }
  .opt:last-child {
    border-bottom: 0;
  }
  .opt {
    flex-wrap: wrap;
  }
  .opt > span {
    flex: 1 1 200px;
    min-width: 0;
  }
  @container panes (max-width: 720px) {
    .panes {
      grid-template-columns: 1fr;
    }
    .list {
      border-right: 0;
      border-bottom: 1px solid var(--line-1);
    }
  }
  .opt.col {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
    min-height: 0;
  }
  .opt.col > span {
    flex: none;
  }
  .hint {
    display: block;
    margin-top: 2px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    word-break: break-word;
  }
  .hint.mono {
    font-family: var(--font-mono);
    word-break: break-all;
  }
  .hint.warn {
    color: var(--warn);
  }
  .acts {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: none;
  }
  .cmdrow {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .cmd {
    flex: 1;
    min-width: 0;
    padding: 4px 8px;
    background: var(--bg-2);
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    font-family: var(--font-mono);
    font-size: var(--fs-xs);
    color: var(--fg-0);
    word-break: break-all;
  }
  .link {
    align-self: flex-start;
    background: none;
    border: 0;
    padding: 0;
    color: var(--focus);
    font: inherit;
    font-size: var(--fs-xs);
    cursor: pointer;
  }
  .link:hover {
    text-decoration: underline;
  }
  .input.addr {
    width: 200px;
  }
  .opt.filter .input {
    flex: 1;
    max-width: 260px;
  }
  .mlist {
    max-height: 360px;
    overflow-y: auto;
  }
  .mrow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
    color: var(--fg-1);
    cursor: pointer;
  }
  .mrow:last-child {
    border-bottom: 0;
  }
  .mrow:hover {
    background: var(--bg-hover);
  }
  .mname {
    color: var(--fg-0);
    white-space: nowrap;
  }
  .mid {
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .tags {
    font-size: var(--fs-2xs);
    color: var(--fg-3);
    white-space: nowrap;
  }
  .err {
    margin-top: 8px;
    color: var(--bad);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
  }
</style>
