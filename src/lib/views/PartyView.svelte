<script lang="ts">
  import { engine, fetchBuildCode, type PartyState, type PartyKind, type PartyBuff, type PartyMod } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { buffLines, modLines, words } from "$lib/party-text";
  import { m } from "$lib/paraglide/messages";

  let party = $state<PartyState | null>(null);
  let importText = $state("");
  let append = $state(false);
  let only = $state("all");

  // PoB's destination dropdown: take everything, or just one kind of buff.
  const DESTINATIONS = $derived<[string, string][]>([
    ["all", m.party_dest_all()],
    ["editPartyMemberStats", m.party_dest_stats()],
    ["editAuras", m.party_dest_auras()],
    ["editCurses", m.party_dest_curses()],
    ["editWarcries", m.party_dest_warcries()],
    ["editLinks", m.party_dest_links()],
    ["enemyCond", m.party_dest_enemy_cond()],
    ["enemyMods", m.party_dest_enemy_mods()],
  ]);
  let advanced = $state(false);
  let fetching = $state(false);

  $effect(() => {
    build.rev;
    if (build.loaded) engine.getParty().then((r) => (party = r)).catch(() => (party = null));
  });

  const boxes = $derived<{ kind: PartyKind; label: string; hint: string }[]>([
    { kind: "auras", label: m.party_box_auras(), hint: m.party_box_auras_hint() },
    { kind: "curses", label: m.party_box_curses(), hint: m.party_box_curses_hint() },
    { kind: "warcries", label: m.party_box_warcries(), hint: "" },
    { kind: "links", label: m.party_box_links(), hint: "" },
    { kind: "partyMemberStats", label: m.party_box_stats(), hint: "" },
    { kind: "enemyConditions", label: m.party_box_enemy_cond(), hint: "" },
    { kind: "enemyMods", label: m.party_box_enemy_mods(), hint: "" },
  ]);

  const buffsOf = (kind: PartyKind): PartyBuff[] | null => {
    const d = party?.details;
    if (!d) return null;
    return kind === "auras" ? d.auras : kind === "curses" ? d.curses : kind === "warcries" ? d.warcries : kind === "links" ? d.links : null;
  };

  const enemyModGroups = $derived.by(() => {
    const groups = new Map<string, PartyMod[]>();
    for (const mod of party?.details.enemyMods ?? []) {
      const key = mod.source ?? "";
      groups.set(key, [...(groups.get(key) ?? []), mod]);
    }
    return [...groups];
  });

  function isEmpty(kind: PartyKind): boolean {
    const d = party?.details;
    if (!d) return true;
    if (kind === "partyMemberStats") return d.memberStats.length === 0 && d.memberFlags.length === 0;
    if (kind === "enemyConditions") return d.enemyConditions.length === 0;
    if (kind === "enemyMods") return d.enemyMods.length === 0;
    return (buffsOf(kind) ?? []).length === 0;
  }

  async function doImport() {
    const text = importText.trim();
    if (!text) return;
    let code = text;
    if (/^https?:\/\//i.test(text)) {
      fetching = true;
      try {
        code = (await fetchBuildCode(text)).code;
      } catch (e) {
        build.error = String(e);
        fetching = false;
        return;
      }
      fetching = false;
    }
    const p = text.startsWith("<") ? { xml: text, append, only } : { code, append, only };
    const r = await build.run(() => engine.partyImport(p));
    if (r) importText = "";
  }
</script>

<div class="page">
  <div class="toolbar">
    <input
      class="input imp"
      placeholder={m.party_import_placeholder()}
      bind:value={importText}
      onkeydown={(e) => e.key === "Enter" && doImport()}
    />
    <button class="btn primary sm" onclick={doImport} disabled={!importText.trim() || build.busy > 0 || fetching}>
      {fetching ? m.party_fetching() : m.party_import()}
    </button>
    <label class="chk small" title={m.party_append_title()}>
      <input type="checkbox" bind:checked={append} />
      {m.party_append()}
    </label>
    <label class="fld-inline" title={m.party_take_title()}>
      <span class="label">{m.party_take()}</span>
      <select class="select sm" bind:value={only}>
        {#each DESTINATIONS as [id, label] (id)}
          <option value={id}>{label}</option>
        {/each}
      </select>
    </label>
    <span class="vr"></span>
    <button class="btn sm ghost" onclick={() => build.run(() => engine.partyClear())} disabled={build.busy > 0}>{m.common_clear()}</button>
    <button
      class="btn sm ghost"
      title={m.party_disable_title()}
      onclick={() => build.run(() => engine.partyDisable())}
      disabled={build.busy > 0}>{m.party_disable()}</button
    >
    <button class="btn sm ghost" title={m.party_rebuild_title()} onclick={() => build.run(() => engine.partyRebuild())} disabled={build.busy > 0}>{m.party_rebuild()}</button>
    <span class="vr"></span>
    <label class="chk small" title={m.party_export_title()}>
      <input type="checkbox" checked={party?.enableExportBuffs ?? false} onchange={(e) => build.run(() => engine.partySetExport((e.target as HTMLInputElement).checked))} />
      {m.party_export()}
    </label>
    <label class="chk small" title={m.party_advanced_title()}>
      <input type="checkbox" bind:checked={advanced} />
      {m.party_advanced()}
    </label>
  </div>
  <div class="scroll">
    {#if !party}
      <div class="dim small pad">{m.party_no_build()}</div>
    {:else}
      {#if !advanced}<p class="dim small note">{m.party_values_note()}</p>{/if}
      <div class="cols">
        {#each boxes as b (b.kind)}
          {@const box = party[b.kind]}
          {@const d = party.details}
          {@const buffs = buffsOf(b.kind)}
          <div class="box" class:empty={advanced ? !box.text : isEmpty(b.kind)}>
            <div class="bhead">
              <span class="blabel">{b.label}</span>
              {#if b.hint}<span class="dim small">{b.hint}</span>{/if}
            </div>
            {#if advanced}
              <textarea
                class="bedit mono"
                rows="6"
                value={box.text}
                spellcheck="false"
                onchange={(e) => build.run(() => engine.setPartyText(b.kind, (e.target as HTMLTextAreaElement).value))}
              ></textarea>
            {:else if isEmpty(b.kind)}
              <div class="bsimple dim">—</div>
            {:else}
              <div class="bsimple">
                {#if b.kind === "curses" && d.curseLimit !== null}
                  <div class="kv"><span class="dim">{m.party_curse_limit()}</span><span class="mono">{d.curseLimit}</span></div>
                {/if}
                {#if buffs}
                  {#each buffs as buff (buff.group + buff.name)}
                    {@const lines = buffLines(buff)}
                    <div class="buff">
                      <div class="bname">
                        <span>{buff.name}</span>
                        {#if buff.effect}<span class="mono dim">{Number(buff.effect.toFixed(1))}%</span>{/if}
                        {#if buff.group === "extraAura"}<span class="tag">{m.party_extra_effects()}</span>{/if}
                        {#if buff.group === "AuraDebuff"}<span class="tag">{m.party_on_enemies()}</span>{/if}
                        {#if buff.isMark}<span class="tag">{m.party_mark()}</span>{/if}
                      </div>
                      {#each lines as text, i (i)}<div class="bline">{text}</div>{/each}
                    </div>
                  {/each}
                {:else if b.kind === "partyMemberStats"}
                  {#each d.memberStats as s (s.name)}
                    <div class="kv"><span class="dim">{words(s.name)}</span><span class="mono">{Number(s.value.toFixed(2))}</span></div>
                  {/each}
                  {#each d.memberFlags as f (f)}<div class="bline">{words(f)}</div>{/each}
                {:else if b.kind === "enemyConditions"}
                  <div class="chips">
                    {#each d.enemyConditions as c (c)}<span class="tag">{words(c)}</span>{/each}
                  </div>
                {:else if b.kind === "enemyMods"}
                  {#each enemyModGroups as [source, mods] (source)}
                    <div class="buff">
                      {#if source}<div class="bname">{source}</div>{/if}
                      {#each modLines(mods) as text, i (i)}<div class="bline">{text}</div>{/each}
                    </div>
                  {/each}
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .page {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-1);
    flex-wrap: wrap;
  }
  .imp {
    flex: 1;
    min-width: 260px;
  }
  .vr {
    width: 1px;
    height: 16px;
    background: var(--line-1);
  }
  .chk {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--fg-1);
    white-space: nowrap;
  }
  input[type="checkbox"] {
    accent-color: var(--fg-0);
  }
  .scroll {
    flex: 1;
    overflow-y: auto;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(380px, 1fr));
    gap: 10px;
    padding: 10px 12px;
    align-items: start;
  }
  .box {
    background: var(--bg-1);
    border: 1px solid var(--line-0);
    border-radius: var(--r-2);
    display: flex;
    flex-direction: column;
  }
  .box.empty {
    opacity: 0.65;
  }
  .bhead {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 7px 10px 4px;
    border-bottom: 1px solid var(--line-0);
  }
  .blabel {
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--fg-2);
  }
  .bsimple {
    padding: 8px 10px;
    font-size: var(--fs-xs);
    line-height: 1.5;
    overflow-wrap: anywhere;
    margin: 0;
    max-height: 300px;
    overflow-y: auto;
  }
  .note {
    margin: 0;
    padding: 10px 12px 0;
  }
  .buff + .buff {
    margin-top: 8px;
  }
  .bname {
    display: flex;
    align-items: baseline;
    gap: 8px;
    color: var(--fg-0);
    font-weight: 600;
  }
  .bline {
    color: var(--fg-1);
    padding-left: 10px;
  }
  .kv {
    display: flex;
    justify-content: space-between;
    gap: 12px;
  }
  .kv + .buff {
    margin-top: 8px;
  }
  .tag {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--fg-2);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    padding: 0 5px;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }
  .bedit {
    background: var(--bg-0);
    border: 0;
    border-top: 1px solid var(--line-1);
    border-radius: 0 0 var(--r-2) var(--r-2);
    color: var(--fg-0);
    font-size: var(--fs-xs);
    line-height: 1.5;
    padding: 6px 8px;
    resize: vertical;
    min-height: 90px;
  }
  .bedit:focus {
    outline: none;
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px 12px;
    max-width: 76ch;
  }
</style>
