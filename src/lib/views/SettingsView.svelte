<script lang="ts">
  import { onMount } from "svelte";
  import { appOptions } from "$lib/state/options.svelte";
  import { mcp, mcpConfigJson } from "$lib/state/mcp.svelte";
  import { game } from "$lib/state/game.svelte";
  import { writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { getVersion } from "@tauri-apps/api/app";
  import { appUpdate } from "$lib/state/update.svelte";
  import { ui, type Theme } from "$lib/state/ui.svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { exportDiagnostics, revealLogs } from "$lib/engine.svelte";
  import { locale, LOCALES, LOCALE_LABEL, type LocalePreference } from "$lib/state/locale.svelte";
  import { decider } from "$lib/state/decide.svelte";
  import { chat } from "$lib/state/chat.svelte";
  import AssistantSettings from "$lib/components/AssistantSettings.svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { m } from "$lib/paraglide/messages";

  type Tone = "ok" | "warn" | "off" | undefined;

  let reportNote = $state("");
  async function saveReport() {
    reportNote = "";
    try {
      const path = await save({ defaultPath: "pob-redux-diagnostics.txt", filters: [{ name: m.settings_report_filetype(), extensions: ["txt"] }] });
      if (!path) return;
      await exportDiagnostics(path);
      reportNote = m.settings_report_saved();
    } catch (e) {
      reportNote = m.settings_report_failed({ error: String(e) });
    }
  }

  const mcpUrl = $derived(mcp.status?.running ? mcp.status.url : null);
  const mcpToken = $derived(mcp.status?.token ?? "");
  const claudeCmd = $derived(
    mcpUrl ? `claude mcp add --transport http pob-redux ${mcpUrl} --header "Authorization: Bearer ${mcpToken}"` : "",
  );
  const jsonCfg = $derived(mcpUrl ? mcpConfigJson(mcp.status) : "");
  let copied = $state("");
  let checked = $state(false);
  let version = $state("");
  getVersion().then((v) => (version = v)).catch(() => {});
  async function copy(key: string, text: string) {
    try {
      await writeText(text);
      copied = key;
      setTimeout(() => (copied = ""), 1200);
    } catch {}
  }

  const themes = $derived<[Theme, string][]>([
    ["system", m.theme_system()],
    ["dark", m.theme_dark()],
    ["wraeclast", m.theme_wraeclast()],
    ["light", m.theme_light()],
  ]);
  const languages = $derived<[LocalePreference, string][]>([
    ["system", m.settings_language_system({ language: LOCALE_LABEL[locale.system] })],
    ...LOCALES.map((l): [LocalePreference, string] => [l, LOCALE_LABEL[l]]),
  ]);
  const scalePresets = [0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2];
  const scales = $derived(scalePresets.includes(ui.scale) ? scalePresets : [...scalePresets, ui.scale].sort((a, b) => a - b));
  const pct = (s: number) => `${Math.round(s * 100)}%`;

  const v = $derived(appOptions.values);

  const dm = $derived(decider.current);
  let keyDraft = $state("");
  let baseDraft = $derived(dm && dm.base_url !== dm.default_base ? dm.base_url : "");
  let modelDraft = $derived(dm && dm.model !== dm.default_model ? dm.model : "");
  async function saveDecideKey() {
    if (!dm || !keyDraft.trim()) return;
    await decider.saveKey(dm.key_id, keyDraft.trim());
    keyDraft = "";
  }

  const assistantTone = $derived<Tone>(chat.ready ? "ok" : "warn");
  const mcpTone = $derived<Tone>(mcp.status?.running ? "ok" : "off");
  const mcpState = $derived(mcp.status?.running ? m.settings_mcp_running_port({ port: mcp.status.port }) : m.status_off());
  const decideTone = $derived<Tone>(decider.enabled ? (decider.ready ? "ok" : "warn") : "off");
  const decideState = $derived(decider.enabled ? (decider.ready ? m.status_on() : m.status_needs_setup()) : m.status_off());

  const sections = $derived<{ id: string; label: string; tone?: Tone }[]>([
    { id: "appearance", label: m.settings_appearance() },
    { id: "numbers", label: m.settings_numbers() },
    ...(game.isPoe2
      ? [
          { id: "assistant", label: m.settings_assistant(), tone: assistantTone },
          { id: "mcp", label: m.settings_mcp(), tone: mcpTone },
          { id: "experimental", label: m.settings_experimental(), tone: decideTone },
        ]
      : []),
    { id: "updates", label: m.settings_updates() },
    { id: "diagnostics", label: m.settings_diagnostics() },
  ]);
  const active = $derived(sections.some((s) => s.id === appOptions.section) ? appOptions.section : "appearance");
  let scroller = $state<HTMLDivElement | null>(null);

  function go(id: string) {
    appOptions.section = id;
    scroller?.scrollTo({ top: 0 });
  }

  function onNavKey(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    const i = sections.findIndex((s) => s.id === active);
    const next = sections[(i + (e.key === "ArrowDown" ? 1 : sections.length - 1)) % sections.length];
    go(next.id);
    (e.currentTarget as HTMLElement).querySelector<HTMLElement>(`[data-section="${next.id}"]`)?.focus();
  }

  function close() {
    appOptions.open = false;
  }

  onMount(() => {
    void decider.init();
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || document.querySelector('[role="alertdialog"]')) return;
      close();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

{#snippet head(title: string, desc: string, status?: string, tone?: Tone)}
  <header class="phead">
    <div class="ptitle">
      <h1>{title}</h1>
      {#if status}
        <span class="badge" class:mono={!tone}>
          {#if tone}<span class="bdot {tone}" aria-hidden="true"></span>{/if}
          {status}
        </span>
      {/if}
    </div>
    <p>{desc}</p>
  </header>
{/snippet}

<div class="settings">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <nav class="snav" aria-label={m.settings_sections()} onkeydown={onNavKey}>
    <div class="label ntitle">{m.settings_title()}</div>
    {#each sections as s (s.id)}
      <button class="nitem" class:on={active === s.id} aria-current={active === s.id ? "page" : undefined} data-section={s.id} onclick={() => go(s.id)}>
        <span>{s.label}</span>
        {#if s.tone === "ok" || s.tone === "warn"}<span class="ndot {s.tone}" aria-hidden="true"></span>{/if}
      </button>
    {/each}
    <button class="btn sm ghost back" onclick={close}>{m.common_close()} <kbd>Esc</kbd></button>
  </nav>

  <div class="sbody" bind:this={scroller}>
    <div class="sinner">
      {#if active === "appearance"}
        {@render head(m.settings_appearance(), m.settings_appearance_desc())}
        <div class="rows">
          <label class="opt">
            <span>
              {m.settings_language()}
              <span class="hint">{m.settings_language_hint()}</span>
            </span>
            <select class="select" value={locale.preference} onchange={(e) => locale.set((e.target as HTMLSelectElement).value as LocalePreference)}>
              {#each languages as [id, label] (id)}
                <option value={id}>{label}</option>
              {/each}
            </select>
          </label>
          <div class="opt">
            <span>{m.settings_theme()}</span>
            <div class="seg" role="radiogroup" aria-label={m.settings_theme()}>
              {#each themes as [id, label] (id)}
                <button role="radio" aria-checked={ui.theme === id} class:on={ui.theme === id} onclick={() => ui.setTheme(id)}>{label}</button>
              {/each}
            </div>
          </div>
          <div class="opt">
            <span>
              {m.settings_contrast()}
              <span class="hint">{m.settings_contrast_hint()}</span>
            </span>
            <div class="ctrast">
              <button class="btn sm ghost" class:on={ui.contrastAuto} aria-pressed={ui.contrastAuto} onclick={() => ui.setContrastAuto(!ui.contrastAuto)}>{m.common_auto()}</button>
              <input
                class="range"
                type="range"
                min="0"
                max="100"
                step="5"
                value={ui.contrastEffective}
                disabled={ui.contrastAuto}
                aria-label={m.settings_contrast_level()}
                oninput={(e) => ui.setContrastLevel(Number((e.target as HTMLInputElement).value))}
              />
              <span class="num ctval">{ui.contrastEffective}%</span>
            </div>
          </div>
          <label class="opt">
            <span>
              {m.settings_scale()}
              <span class="hint">{m.settings_scale_hint()}</span>
              {#if ui.scaleLimited}
                <span class="hint warn">{m.settings_scale_limited({ applied: pct(ui.scaleApplied), wanted: pct(ui.scale) })}</span>
              {/if}
            </span>
            <select class="select" value={String(ui.scale)} onchange={(e) => ui.setScale(Number((e.target as HTMLSelectElement).value))}>
              {#each scales as s (s)}
                <option value={String(s)}>{pct(s)}</option>
              {/each}
            </select>
          </label>
        </div>
      {:else if active === "numbers"}
        {@render head(m.settings_numbers(), m.settings_numbers_desc())}
        {#if v}
          <div class="cols">
            <div>
              <h2 class="label ghead">{m.settings_numbers_format()}</h2>
              <div class="rows">
                <label class="opt">
                  <span>{m.settings_thousands_show()}</span>
                  <input
                    class="switch"
                    type="checkbox"
                    role="switch"
                    checked={v.showThousandsSeparators}
                    onchange={(e) => appOptions.set({ showThousandsSeparators: (e.target as HTMLInputElement).checked })}
                  />
                </label>
                <label class="opt">
                  <span>{m.settings_thousands_separator()}</span>
                  <input
                    class="input chr"
                    maxlength="1"
                    value={v.thousandsSeparator}
                    onchange={(e) => appOptions.set({ thousandsSeparator: (e.target as HTMLInputElement).value || "," })}
                  />
                </label>
                <label class="opt">
                  <span>{m.settings_decimal_separator()}</span>
                  <input
                    class="input chr"
                    maxlength="1"
                    value={v.decimalSeparator}
                    onchange={(e) => appOptions.set({ decimalSeparator: (e.target as HTMLInputElement).value || "." })}
                  />
                </label>
              </div>
            </div>
            <div>
              <h2 class="label ghead">{m.settings_numbers_defaults()}</h2>
              <div class="rows">
                <label class="opt">
                  <span>{m.settings_gem_quality()}</span>
                  <input
                    class="input num"
                    type="number"
                    min="0"
                    max="20"
                    value={v.defaultGemQuality}
                    onchange={(e) => appOptions.set({ defaultGemQuality: Math.max(0, Math.min(20, Number((e.target as HTMLInputElement).value) || 0)) })}
                  />
                </label>
                <label class="opt">
                  <span>{m.settings_char_level()}</span>
                  <input
                    class="input num"
                    type="number"
                    min="1"
                    max="100"
                    value={v.defaultCharLevel}
                    onchange={(e) => appOptions.set({ defaultCharLevel: Math.max(1, Math.min(100, Number((e.target as HTMLInputElement).value) || 1)) })}
                  />
                </label>
                <label class="opt">
                  <span>{m.settings_affix_quality()}</span>
                  <select class="select" value={String(v.defaultItemAffixQuality)} onchange={(e) => appOptions.set({ defaultItemAffixQuality: Number((e.target as HTMLSelectElement).value) })}>
                    <option value="0">{m.settings_affix_worst()}</option>
                    <option value="0.25">25%</option>
                    <option value="0.5">{m.settings_affix_average()}</option>
                    <option value="0.75">75%</option>
                    <option value="1">{m.settings_affix_best()}</option>
                  </select>
                </label>
              </div>
            </div>
          </div>
          <p class="note">{m.settings_numbers_note()}</p>
        {:else}
          <p class="note">{m.settings_numbers_waiting()}</p>
        {/if}
      {:else if active === "assistant"}
        {@render head(m.settings_assistant(), m.settings_assistant_desc(), chat.current?.label ?? "", assistantTone)}
        <AssistantSettings />
        <h2 class="label ghead">{m.assistant_changes()}</h2>
        <div class="rows">
          <label class="opt">
            <span>
              {m.assistant_ask_first()}
              <span class="hint">{m.assistant_ask_first_hint()}</span>
            </span>
            <input class="switch" type="checkbox" role="switch" checked={chat.askFirst} onchange={(e) => chat.setAskFirst((e.target as HTMLInputElement).checked)} />
          </label>
        </div>
        <p class="note">{m.provider_intro_agents()}</p>
      {:else if active === "mcp"}
        {@render head(m.settings_mcp(), m.settings_mcp_desc(), mcpState, mcpTone)}
        <h2 class="label ghead">{m.settings_mcp_server()}</h2>
        <div class="rows">
          <label class="opt">
            <span>
              {m.settings_mcp_enable()}
              <span class="hint">{m.settings_mcp_enable_hint()}</span>
            </span>
            <input class="switch" type="checkbox" role="switch" checked={mcp.enabled} disabled={mcp.busy} onchange={(e) => mcp.setEnabled((e.target as HTMLInputElement).checked)} />
          </label>
          <label class="opt">
            <span>{m.settings_mcp_port()}</span>
            <input
              class="input num"
              type="number"
              min="1024"
              max="65535"
              value={mcp.port}
              disabled={mcp.busy}
              onchange={(e) => mcp.setPort(Number((e.target as HTMLInputElement).value))}
            />
          </label>
          {#if mcp.status?.error}
            <div class="opt err mono">{mcp.status.error}</div>
          {/if}
        </div>
        {#if mcpUrl}
          <h2 class="label ghead">{m.settings_mcp_connect()}</h2>
          <div class="rows">
            <div class="opt col">
              <div class="row">
                <span class="dim">URL</span>
                <code class="mono selectable">{mcpUrl}</code>
                <button class="btn sm ghost" onclick={() => copy("url", mcpUrl)}>{copied === "url" ? m.common_copied() : m.common_copy()}</button>
              </div>
              <div class="row">
                <span class="dim">Token</span>
                <code class="mono selectable">{mcpToken}</code>
                <button class="btn sm ghost" onclick={() => copy("token", mcpToken)}>{copied === "token" ? m.common_copied() : m.common_copy()}</button>
              </div>
              <div class="row">
                <span class="dim">Claude Code</span>
                <code class="mono selectable">{claudeCmd}</code>
                <button class="btn sm ghost" onclick={() => copy("cmd", claudeCmd)}>{copied === "cmd" ? m.common_copied() : m.common_copy()}</button>
              </div>
              <div class="row">
                <span class="dim">JSON</span>
                <code class="mono selectable">{jsonCfg}</code>
                <button class="btn sm ghost" onclick={() => copy("json", jsonCfg)}>{copied === "json" ? m.common_copied() : m.common_copy()}</button>
              </div>
            </div>
          </div>
        {/if}
      {:else if active === "experimental"}
        {@render head(m.settings_experimental(), m.settings_experimental_desc(), decideState, decideTone)}
        <div class="rows master" class:live={decider.enabled}>
          <label class="opt">
            <span>
              {m.experimental_enable()}
              <span class="hint">{m.experimental_enable_hint()}</span>
            </span>
            <input class="switch" type="checkbox" role="switch" checked={decider.enabled} onchange={(e) => decider.setEnabled((e.target as HTMLInputElement).checked)} />
          </label>
        </div>
        {#if decider.enabled && dm && decider.status}
          <h2 class="label ghead">{m.experimental_setup()}</h2>
          <div class="rows">
            <div class="opt">
              <span>
                {m.experimental_backend()}
                <span class="hint">{m.experimental_backend_hint()}</span>
              </span>
              <div class="seg" role="radiogroup" aria-label={m.experimental_backend()}>
                {#each decider.status.backends as b (b.id)}
                  <button role="radio" aria-checked={b.id === dm.id} class:on={b.id === dm.id} disabled={decider.busy} onclick={() => decider.select(b.id)}>{b.label}</button>
                {/each}
              </div>
            </div>
            <div class="opt">
              <span>
                {m.experimental_key()}
                {#if dm.has_key}
                  <span class="hint mono">···{dm.hint}</span>
                {/if}
                {#if !dm.has_key && !dm.needs_key}
                  <span class="hint">{m.experimental_key_optional()}</span>
                {/if}
                {#if dm.keys_url}
                  <button class="link" onclick={() => openUrl(dm.keys_url!)}>{m.provider_get_key()}</button>
                {/if}
              </span>
              <div class="row">
                <input
                  class="input key"
                  type="password"
                  placeholder={dm.has_key ? m.provider_replace_key() : m.provider_paste_key()}
                  bind:value={keyDraft}
                  onkeydown={(e) => e.key === "Enter" && saveDecideKey()}
                />
                <button class="btn sm" disabled={decider.busy || !keyDraft.trim()} onclick={saveDecideKey}>{m.common_save()}</button>
                {#if dm.has_key}
                  <button class="btn sm ghost" disabled={decider.busy} onclick={() => decider.removeKey(dm.key_id)}>{m.provider_remove()}</button>
                {/if}
              </div>
            </div>
            <div class="opt">
              <span>
                {m.experimental_address()}
                <span class="hint">{dm.id === "openrouter" ? m.experimental_openrouter_models() : m.experimental_address_hint()}</span>
              </span>
              <div class="row">
                <input class="input mono addr" bind:value={baseDraft} placeholder={dm.default_base} aria-label={m.experimental_address()} />
                <input class="input mono model" bind:value={modelDraft} placeholder={dm.default_model} aria-label={m.experimental_model()} />
                <button class="btn sm ghost" disabled={decider.busy} onclick={() => decider.configure(dm.id, baseDraft, modelDraft)}>{m.common_save()}</button>
              </div>
            </div>
            <div class="opt">
              <span>
                {m.experimental_connection()}
                {#if decider.test}
                  <span class="hint mono" style:color={decider.test.ok ? "var(--ok)" : "var(--bad)"}>{decider.test.text}</span>
                {:else if !decider.ready}
                  <span class="hint warn">{m.experimental_not_ready()}</span>
                {/if}
              </span>
              <button class="btn sm ghost" disabled={decider.testing || decider.busy || !decider.ready} onclick={() => decider.runTest()}>
                {decider.testing ? m.experimental_testing() : m.experimental_test()}
              </button>
            </div>
            {#if decider.error}
              <div class="opt err mono">{decider.error}</div>
            {/if}
          </div>
          <h2 class="label ghead">{m.experimental_features()}</h2>
          <div class="rows">
            <label class="opt">
              <span>
                {m.experimental_routing()}
                <span class="hint">{m.experimental_routing_hint()}</span>
              </span>
              <input class="switch" type="checkbox" role="switch" checked={decider.routing} onchange={(e) => decider.setRouting((e.target as HTMLInputElement).checked)} />
            </label>
          </div>
          <p class="note">{m.experimental_privacy()}</p>
        {/if}
      {:else if active === "updates"}
        {@render head(m.settings_updates(), m.settings_updates_desc(), version)}
        <div class="rows">
          <div class="opt">
            <span>
              {m.settings_version()}
              <span class="hint">{m.settings_version_hint()}</span>
            </span>
            <div class="row">
              {#if appUpdate.phase === "available"}
                <span>{m.update_available()} <span class="mono" style:color="var(--ok)">{appUpdate.version}</span> {m.update_available_suffix()}</span>
                {#if appUpdate.method === "self"}
                  <button class="btn sm" onclick={() => appUpdate.install()}>{m.update_install()}</button>
                {:else if appUpdate.method === "aur"}
                  <span class="dim">{m.update_aur_before()} <span class="mono">pob-redux-bin</span> {m.update_aur_after()}</span>
                {:else}
                  <button class="btn sm" onclick={() => appUpdate.openReleases()} title={m.update_download_title()}>{m.update_download()}</button>
                {/if}
              {:else if appUpdate.phase === "downloading"}
                <span class="dim">
                  {m.update_downloading()} <span class="mono">{appUpdate.version}</span>
                  {#if appUpdate.progress !== null}<span class="mono">{appUpdate.progress}%</span>{/if}
                </span>
              {:else if appUpdate.phase === "ready"}
                <span class="mono" style:color="var(--ok)">{m.settings_version_ready({ version: appUpdate.version ?? "" })}</span>
                <button class="btn sm" onclick={() => appUpdate.restart()}>{m.update_restart()}</button>
              {:else}
                {#if appUpdate.phase === "checking"}
                  <span class="dim">{m.settings_version_checking()}</span>
                {:else if appUpdate.phase === "error"}
                  <span class="mono" style:color="var(--bad)" title={appUpdate.error}>{m.settings_version_failed()}</span>
                {:else if checked}
                  <span class="dim">{m.settings_version_current()}</span>
                {/if}
                <button
                  class="btn sm ghost"
                  onclick={async () => {
                    await appUpdate.check(true);
                    checked = true;
                  }}
                  disabled={appUpdate.phase === "checking"}
                >
                  {m.settings_check_updates()}
                </button>
              {/if}
            </div>
          </div>
        </div>
      {:else if active === "diagnostics"}
        {@render head(m.settings_diagnostics(), m.settings_diagnostics_desc())}
        <div class="rows">
          <div class="opt">
            <span>
              {m.settings_report()}
              <span class="hint">{m.settings_report_hint()}</span>
            </span>
            <div class="row">
              {#if reportNote}<span class="dim">{reportNote}</span>{/if}
              <button class="btn sm ghost" onclick={() => revealLogs().catch((e) => (reportNote = String(e)))}>{m.settings_log_folder()}</button>
              <button class="btn sm ghost" onclick={saveReport}>{m.settings_save_report()}</button>
            </div>
          </div>
        </div>
      {/if}
    </div>
  </div>
</div>

<style>
  .settings {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .snav {
    width: 210px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 14px 8px 10px;
    border-right: 1px solid var(--line-1);
    background: var(--bg-1);
    overflow-y: auto;
  }
  .ntitle {
    padding: 0 10px 8px;
  }
  .nitem {
    appearance: none;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    border: 0;
    border-radius: var(--r-1);
    background: transparent;
    color: var(--fg-2);
    font-size: var(--fs-md);
    text-align: left;
    padding: 7px 10px;
  }
  .nitem:hover {
    color: var(--fg-0);
    background: var(--bg-hover);
  }
  .nitem.on {
    color: var(--fg-0);
    background: var(--bg-active);
    box-shadow: inset 2px 0 0 var(--fg-0);
  }
  .ndot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex: none;
  }
  .ndot.ok {
    background: var(--ok);
  }
  .ndot.warn {
    background: var(--warn);
  }
  .back {
    margin-top: auto;
    align-self: flex-start;
  }
  .sbody {
    position: relative;
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    background: var(--bg-0);
  }
  .sinner {
    max-width: 1080px;
    padding: 26px 36px 48px;
  }
  .phead {
    padding-bottom: 16px;
    margin-bottom: 20px;
    border-bottom: 1px solid var(--line-1);
  }
  .ptitle {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .phead h1 {
    margin: 0;
    font-size: var(--fs-xl);
    font-weight: 600;
    color: var(--fg-0);
  }
  .phead p {
    margin: 6px 0 0;
    font-size: var(--fs-sm);
    color: var(--fg-2);
    max-width: 640px;
  }
  .badge {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px 9px;
    border: 1px solid var(--line-1);
    border-radius: 999px;
    background: var(--bg-1);
    font-size: var(--fs-xs);
    color: var(--fg-1);
    white-space: nowrap;
  }
  .bdot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--fg-3);
  }
  .bdot.ok {
    background: var(--ok);
  }
  .bdot.warn {
    background: var(--warn);
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(380px, 1fr));
    gap: 0 20px;
    align-items: start;
  }
  .cols > div > .ghead {
    margin-top: 0;
  }
  .ghead {
    margin: 22px 2px 8px;
  }
  .phead + .ghead {
    margin-top: 0;
  }
  .rows {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    background: var(--bg-1);
  }
  .rows.master {
    border-color: var(--line-2);
  }
  .rows.master .opt {
    padding: 14px 14px;
    font-size: var(--fs-md);
    color: var(--fg-0);
  }
  .rows.master.live {
    border-color: var(--fg-3);
  }
  .opt {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    min-height: 48px;
    padding: 11px 14px;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
    color: var(--fg-1);
  }
  .opt:last-child {
    border-bottom: 0;
  }
  .note {
    margin: 10px 2px 0;
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .ctrast {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .ctrast .range {
    width: 150px;
    height: 14px;
    accent-color: var(--fg-1);
  }
  .ctrast .range:disabled {
    opacity: var(--fade-off);
  }
  .ctval {
    min-width: 34px;
    text-align: right;
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .seg {
    display: inline-flex;
    flex: none;
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow: hidden;
  }
  .seg button {
    padding: 3px 10px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    background: transparent;
    border: 0;
    border-right: 1px solid var(--line-1);
  }
  .seg button:last-child {
    border-right: 0;
  }
  .seg button.on {
    color: var(--fg-0);
    background: var(--bg-3);
  }
  .input.chr {
    width: 40px;
    text-align: center;
  }
  .input.num {
    width: 70px;
    text-align: right;
  }
  .input.key,
  .input.addr {
    width: 180px;
  }
  .input.model {
    width: 110px;
  }
  .hint {
    display: block;
    color: var(--fg-2);
    font-size: var(--fs-xs);
    max-width: 600px;
    margin-top: 2px;
  }
  .hint.warn {
    color: var(--warn);
  }
  .link {
    display: block;
    margin-top: 2px;
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
  .opt.err {
    color: var(--bad);
    font-size: var(--fs-xs);
    white-space: pre-wrap;
  }
  .opt.col {
    flex-direction: column;
    align-items: stretch;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .row > .dim {
    width: 84px;
    flex: none;
  }
  .row > code {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
</style>
