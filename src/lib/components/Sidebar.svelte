<script lang="ts">
  import { tick } from "svelte";
  import PobText from "./PobText.svelte";
  import BreakdownPanel from "./BreakdownPanel.svelte";
  import Icon from "./Icon.svelte";
  import MinionLibrary from "./MinionLibrary.svelte";
  import { engine, type BreakdownSection } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { groupSidebar, type SidebarSection } from "$lib/sidebar-groups";
  import { parsePobText, stripPobText } from "$lib/pobtext";
  import { m } from "$lib/paraglide/messages";

  let libraryOpen = $state(false);

  const COLLAPSED_KEY = "pob-redux:stats-collapsed";
  let collapsed = $state<Set<string>>(new Set());
  try {
    collapsed = new Set(JSON.parse(localStorage.getItem(COLLAPSED_KEY) ?? "[]"));
  } catch {}
  function toggleGroup(key: string) {
    const next = new Set(collapsed);
    next.has(key) ? next.delete(key) : next.add(key);
    collapsed = next;
    try {
      localStorage.setItem(COLLAPSED_KEY, JSON.stringify([...next]));
    } catch {}
  }

  // What a collapsed group still shows in its header: the first of these stats PoB lists, or all of them when `all`.
  const HEADLINE: Record<string, { stats: string[]; all?: boolean }> = {
    offence: { stats: ["FullDPS", "CombinedDPS", "TotalDPS", "TotalDotDPS", "AverageDamage", "AverageHit"] },
    attributes: { stats: ["Str", "Dex", "Int"], all: true },
    resources: { stats: ["Life", "EnergyShield", "Mana"] },
    mitigation: { stats: ["TotalEHP"] },
    resistances: { stats: ["FireResist", "ColdResist", "LightningResist", "ChaosResist"], all: true },
    fulldps: { stats: ["FullDPS"] },
  };
  function headline(sec: SidebarSection): { text: string; color: string | null }[] {
    const h = HEADLINE[sec.key];
    if (!h) return [];
    const byStat = new Map(sec.items.filter((i) => i.row.stat && i.row.rhs).map((i) => [i.row.stat!, i.row]));
    const found = h.stats.flatMap((k) => (byStat.has(k) ? [byStat.get(k)!] : []));
    if (!h.all) return found.slice(0, 1).map((r) => ({ text: r.rhs!, color: null }));
    // Several values share one line: drop PoB's "(+40%)" overcap and take each label's colour.
    return found.map((r) => ({
      text: stripPobText(r.rhs).replace(/\s*\(.*\)\s*$/, ""),
      color: parsePobText(r.lhs).find((sp) => sp.text.trim())?.color ?? null,
    }));
  }

  // breakdown popup for hovered/pinned stat rows
  let bd = $state<{ sections: BreakdownSection[]; row: number; y: number; pinned: boolean; ready: boolean } | null>(null);
  let bdPop = $state<HTMLElement>();
  let bdTimer = 0;
  const bdCache = new Map<string, BreakdownSection[]>();

  async function placeBreakdown(row: number, clientY: number) {
    await tick();
    if (!bd || bd.row !== row || !bdPop) return;
    const height = bdPop.getBoundingClientRect().height;
    const y = Math.max(40, Math.min(clientY - 40, window.innerHeight - height - 12));
    bd = { ...bd, y, ready: true };
  }

  function rowBreakdown(clientY: number, rowIndex: number, pin: boolean) {
    clearTimeout(bdTimer);
    if (pin && bd?.pinned && bd.row === rowIndex) {
      bd = null;
      return;
    }
    const y = Math.max(40, clientY - 40);
    const key = `${rowIndex}:${build.rev}`;
    const apply = (sections: BreakdownSection[]) => {
      bd = { sections, row: rowIndex, y, pinned: pin || (bd?.pinned && bd.row === rowIndex) || false, ready: false };
      void placeBreakdown(rowIndex, clientY);
    };
    const cached = bdCache.get(key);
    if (cached) {
      apply(cached);
      return;
    }
    bdTimer = window.setTimeout(async () => {
      try {
        const r = await engine.sidebarBreakdown(rowIndex);
        bdCache.set(key, r.sections);
        apply(r.sections);
      } catch {
        /* row changed under us */
      }
    }, pin ? 0 : 140);
  }

  function rowLeave() {
    clearTimeout(bdTimer);
    if (bd && !bd.pinned) bd = null;
  }

  $effect(() => {
    build.rev;
    bdCache.clear();
    bd = null;
  });

  const info = $derived(build.info);
  const side = $derived(build.sidebar);
  const sections = $derived(side ? groupSidebar(side.rows) : []);
  const cls = $derived(build.classes.find((c) => c.id === info?.classId));
  const groups = $derived(build.skills?.socketGroups ?? []);
  const mainGroup = $derived(groups.find((g) => g.index === info?.mainSocketGroup));
  const mainSkill = $derived(mainGroup?.skills.find((s) => s.index === (mainGroup?.mainActiveSkill ?? 1)) ?? mainGroup?.skills[0]);

  function patchMainSkill(patch: Parameters<typeof engine.setMainSkillOptions>[1]) {
    if (mainGroup) build.run(() => engine.setMainSkillOptions(mainGroup.index, patch));
  }

  let levelDraft = $state("");
  $effect(() => {
    if (info) levelDraft = String(info.level);
  });

  // loadouts: PoB's named tree+items+skills+config combos
  let loadouts = $state<{ loadouts: string[]; active: string | null }>({ loadouts: [], active: null });
  let loEdit = $state<{ mode: "new" | "copy" | "rename"; draft: string } | null>(null);
  $effect(() => {
    build.rev;
    if (build.loaded) engine.getLoadouts().then((r) => (loadouts = r)).catch(() => {});
  });
  function loCommit() {
    const e = loEdit;
    loEdit = null;
    if (!e) return;
    const name = e.draft.trim();
    if (!name) return;
    if (e.mode === "new") build.run(() => engine.newLoadout(name));
    else if (e.mode === "copy" && loadouts.active) build.run(() => engine.copyLoadout(loadouts.active!, name));
    else if (e.mode === "rename" && loadouts.active && name !== loadouts.active) build.run(() => engine.renameLoadout(loadouts.active!, name));
  }

  function commitLevel() {
    const n = Number(levelDraft);
    if (info && Number.isFinite(n) && n !== info.level) build.setLevel(n);
  }

  // Click the build name to rename it; Enter or blur commits, Escape cancels.
  let nameEdit = $state<string | null>(null);
  function commitName() {
    const name = nameEdit?.trim() ?? "";
    nameEdit = null;
    if (name && name !== info?.name) build.rename(name);
  }

  function onClass(e: Event) {
    build.selectClass(Number((e.target as HTMLSelectElement).value), 0);
  }
  function onAsc(e: Event) {
    build.chooseAscendancy(Number((e.target as HTMLSelectElement).value));
  }
  function onSecondaryAsc(e: Event) {
    build.selectClass(undefined, undefined, Number((e.target as HTMLSelectElement).value));
  }
  function onMainSkill(e: Event) {
    build.setMainSkill(Number((e.target as HTMLSelectElement).value));
  }

  // PoB emits "label:" / "value" pairs plus header rows (only lhs) and spacer
  // rows (no text). Header detection: lhs without a trailing colon.
  function kind(r: { lhs: string | null; rhs: string | null; h: number; align: string | null }) {
    if (!r.lhs && !r.rhs) return "space";
    if (r.align === "CENTER_X") return "center";
    if (r.lhs && !r.rhs) return "head";
    return "row";
  }
</script>

<aside class="sidebar">
  {#if info}
    <section class="head">
      <div class="buildname">
        <span class="label">{m.sidebar_build()}</span>
        {#if nameEdit !== null}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="input"
            bind:value={nameEdit}
            autofocus
            onblur={commitName}
            onkeydown={(e) => {
              if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              if (e.key === "Escape") (nameEdit = null);
            }}
          />
        {:else}
          <button
            class="bname"
            onclick={() => (nameEdit = info?.name ?? "")}
            title={(info.file ? `${info.file}\n` : m.sidebar_not_saved()) + m.sidebar_rename_title()}
          >
            <span class="bn">{info.name}</span>
            {#if info.unsaved}<span class="unsaved" title={m.sidebar_unsaved()}>●</span>{/if}
          </button>
        {/if}
      </div>
      {#if loadouts.loadouts.length}
        <div class="field">
          <span class="label">{m.sidebar_loadout()}</span>
          {#if loEdit}
            <input
              class="input"
              placeholder={loEdit.mode === "new" ? m.sidebar_loadout_new_placeholder() : loEdit.mode === "copy" ? m.sidebar_loadout_copy_placeholder() : m.sidebar_loadout_rename_placeholder()}
              bind:value={loEdit.draft}
              onblur={loCommit}
              onkeydown={(e) => {
                if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                if (e.key === "Escape") (loEdit = null);
              }}
            />
          {:else}
            <div class="lorow">
              <select
                class="select"
                value={loadouts.active ?? ""}
                onchange={(e) => build.run(() => engine.selectLoadout((e.target as HTMLSelectElement).value))}
                disabled={build.busy > 0}
                title={m.sidebar_loadout_title()}
              >
                {#if !loadouts.active}<option value="">—</option>{/if}
                {#each loadouts.loadouts as l}
                  <option value={l}>{stripPobText(l)}</option>
                {/each}
              </select>
              <button class="loact" title={m.sidebar_loadout_new()} aria-label={m.sidebar_loadout_new()} onclick={() => (loEdit = { mode: "new", draft: "" })}>
                <Icon name="plus" size={13} />
              </button>
              <button
                class="loact"
                title={m.sidebar_loadout_copy()}
                aria-label={m.sidebar_loadout_copy()}
                disabled={!loadouts.active}
                onclick={() => (loEdit = { mode: "copy", draft: m.sidebar_loadout_copy_suffix({ name: loadouts.active ?? "" }) })}
              >
                <Icon name="copy" size={13} />
              </button>
              <button
                class="loact"
                title={m.sidebar_loadout_rename()}
                aria-label={m.sidebar_loadout_rename()}
                disabled={!loadouts.active}
                onclick={() => (loEdit = { mode: "rename", draft: loadouts.active ?? "" })}
              >
                <Icon name="pencil" size={13} />
              </button>
              <button
                class="loact danger"
                title={m.sidebar_loadout_delete_title()}
                aria-label={m.sidebar_loadout_delete()}
                disabled={loadouts.loadouts.length <= 1 || !loadouts.active}
                onclick={() => loadouts.active && build.run(() => engine.deleteLoadout(loadouts.active!))}
              >
                <Icon name="trash" size={13} />
              </button>
            </div>
          {/if}
        </div>
      {/if}
      <div class="row2">
        <label class="field">
          <span class="label">{m.sidebar_class()}</span>
          <select class="select" value={info.classId} onchange={onClass} disabled={build.busy > 0}>
            {#each build.classes as c}
              <option value={c.id}>{c.name}</option>
            {/each}
          </select>
        </label>
        <label class="field">
          <span class="label">{m.sidebar_ascendancy()}</span>
          <select class="select" value={info.ascendClassId} onchange={onAsc} disabled={build.busy > 0}>
            <option value={0}>{m.sidebar_ascendancy_none()}</option>
            {#each cls?.ascendancies ?? [] as a}
              <option value={a.id}>{a.name}</option>
            {/each}
          </select>
        </label>
      </div>
      {#if build.secondaryAscendancies.length}
        <div class="row2">
          <label class="field wide">
            <span class="label">{m.sidebar_second_ascendancy()}</span>
            <select class="select" value={info.secondaryAscendClassId ?? 0} onchange={onSecondaryAsc} disabled={build.busy > 0}>
              <option value={0}>{m.sidebar_ascendancy_none()}</option>
              {#each build.secondaryAscendancies as a}
                <option value={a.id}>{a.name}</option>
              {/each}
            </select>
          </label>
        </div>
      {/if}
      <div class="row2">
        <div class="field lvl">
          <span class="label">{m.sidebar_level()}</span>
          <div class="lvlrow">
            <input
              class="input num"
              type="number"
              min="1"
              max="100"
              bind:value={levelDraft}
              onchange={commitLevel}
              onkeydown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
            />
            <button
              class="auto"
              class:on={info.levelAuto}
              title={info.levelAuto ? m.sidebar_level_auto_on() : m.sidebar_level_auto_off()}
              onclick={() => build.setLevelAuto(!info.levelAuto)}
            >{m.sidebar_level_auto()}</button>
          </div>
        </div>
        <div class="field points" title={info.points.requiredLevelText ?? ""}>
          <span class="label">{m.sidebar_points()}</span>
          <div class="pts num">
            <span class:over={info.points.used > info.points.max}>{info.points.used}<span class="dim">/{info.points.max}</span></span>
            <span class="sep">·</span>
            <span class="asc" class:over={info.points.ascUsed > info.points.ascMax}>{info.points.ascUsed}<span class="dim">/{info.points.ascMax}</span></span>
            <span class="dim label2">{m.sidebar_points_asc()}</span>
          </div>
        </div>
      </div>
      {#if info.points.weaponSet1Used || info.points.weaponSet2Used}
        {@const wsMax = info.points.weaponSetMax}
        <div class="row2">
          <div class="field wide" title={m.sidebar_weapon_set_title({ max: wsMax })}>
            <span class="label">{m.sidebar_weapon_set_points()}</span>
            <div class="pts num">
              <span class:over={info.points.weaponSet1Used > wsMax}><span class="set1">I</span> {info.points.weaponSet1Used}<span class="dim">/{wsMax}</span></span>
              <span class="sep">·</span>
              <span class:over={info.points.weaponSet2Used > wsMax}><span class="set2">II</span> {info.points.weaponSet2Used}<span class="dim">/{wsMax}</span></span>
            </div>
          </div>
        </div>
      {/if}
      <label class="field">
        <span class="label">{m.sidebar_main_skill()}</span>
        <select class="select" value={info.mainSocketGroup} onchange={onMainSkill} disabled={groups.length === 0 || build.busy > 0}>
          {#if groups.length === 0}
            <option value={0}>{m.sidebar_no_skills()}</option>
          {/if}
          {#each groups as g}
            <option value={g.index}>{g.grantedBy?.kind === "mechanic" ? "◈ " : g.grantedBy?.kind === "node" ? "✦ " : g.grantedBy ? "⚔ " : ""}{stripPobText(g.displayLabel ?? g.label ?? m.sidebar_group_fallback({ index: g.index }))}{g.duplicateOf ? m.sidebar_group_item_copy({ source: g.duplicateOf }) : ""}</option>
          {/each}
        </select>
      </label>
      {#if mainGroup && mainGroup.skills.length > 1}
        <label class="field">
          <span class="label">{m.sidebar_active_skill()}</span>
          <select class="select" value={mainGroup.mainActiveSkill ?? 1} onchange={(e) => patchMainSkill({ mainActiveSkill: Number((e.target as HTMLSelectElement).value) })}>
            {#each mainGroup.skills as s}
              <option value={s.index}>{s.name}</option>
            {/each}
          </select>
        </label>
      {/if}
      {#if mainSkill?.statSets?.length}
        <label class="field">
          <span class="label">{m.sidebar_stat_set()}</span>
          <select class="select" value={mainSkill.statSet ?? 1} onchange={(e) => patchMainSkill({ statSet: Number((e.target as HTMLSelectElement).value) })}>
            {#each mainSkill.statSets as label, i}
              <option value={i + 1}>{label}</option>
            {/each}
          </select>
        </label>
      {/if}
      {#if mainSkill?.parts?.length}
        <label class="field">
          <span class="label">{m.sidebar_skill_part()}</span>
          <select class="select" value={mainSkill.part ?? 1} onchange={(e) => patchMainSkill({ part: Number((e.target as HTMLSelectElement).value) })}>
            {#each mainSkill.parts as part, i}
              <option value={i + 1}>{part.name}</option>
            {/each}
          </select>
        </label>
      {/if}
      {#if mainSkill?.minions?.length}
        <label class="field">
          <span class="label">{m.sidebar_minion()}</span>
          <select class="select" value={mainSkill.minion ?? mainSkill.minions[0].id} onchange={(e) => patchMainSkill({ minionId: (e.target as HTMLSelectElement).value })}>
            {#each mainSkill.minions as minion}
              <option value={minion.id}>{minion.name}</option>
            {/each}
          </select>
        </label>
      {/if}
      {#if mainSkill?.minionLibrary}
        <button
          class="btn sm wide"
          title={mainSkill.minionLibrary === "beast" ? m.sidebar_manage_beasts_title() : m.sidebar_manage_spectres_title()}
          onclick={() => (libraryOpen = true)}
        >
          {mainSkill.minionLibrary === "beast" ? m.sidebar_manage_beasts() : m.sidebar_manage_spectres()}
        </button>
      {/if}
    </section>

    {#if libraryOpen}
      <MinionLibrary kind={mainSkill?.minionLibrary ?? "spectre"} onclose={() => (libraryOpen = false)} />
    {/if}

    <div class="stats" class:busy={build.busy > 0}>
      {#if side}
        {#each sections as sec (sec.key)}
          {@const open = !sec.label || !collapsed.has(sec.key)}
          <section class="scard">
            {#if sec.label}
              <button class="cardhead" aria-expanded={open} onclick={() => toggleGroup(sec.key)}>
                <span class="caret" class:open>▸</span>
                <span class="cardname">{sec.label}</span>
                {#if !open}
                  <span class="cardsum num">
                    {#each headline(sec) as v, i}{#if i}<span class="sep">/</span>{/if}<PobText text={v.text} defaultColor={v.color} />{/each}
                  </span>
                {/if}
              </button>
            {/if}
            {#if open}
              <div class="cardbody">
                {#each sec.items as { row: r, index: rowIndex } (rowIndex)}
                  {@const k = kind(r)}
                  {#if k === "space"}
                    <div class="space"></div>
                  {:else if k === "head"}
                    <div class="shead"><PobText text={r.lhs} /></div>
                  {:else if k === "center"}
                    <div class="scenter"><PobText text={r.lhs} defaultColor="var(--fg-2)" /></div>
                  {:else}
                    {@const pinned = bd?.pinned && bd.row === rowIndex + 1}
                    <div
                      class="srow"
                      class:hasbd={r.hasBreakdown}
                      aria-pressed={pinned}
                      role="button"
                      tabindex={r.hasBreakdown ? 0 : -1}
                      onmouseenter={(e) => r.hasBreakdown && rowBreakdown(e.clientY, rowIndex + 1, false)}
                      onmouseleave={rowLeave}
                      onclick={(e) => r.hasBreakdown && rowBreakdown(e.clientY, rowIndex + 1, true)}
                      onkeydown={(e) => e.key === "Enter" && r.hasBreakdown && rowBreakdown(200, rowIndex + 1, true)}
                    >
                      <span class="k"><PobText text={r.lhs?.replace(/:\s*$/, "")} defaultColor="var(--fg-1)" /></span>
                      <span class="svalue">
                        <span class="v num"><PobText text={r.rhs} /></span>
                        {#if pinned}<span class="pinmark" title={m.sidebar_breakdown_pinned()}><Icon name="push-pin" size={11} /></span>{/if}
                      </span>
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          </section>
        {/each}
        {#if side.warnings.length}
          {@const open = !collapsed.has("warnings")}
          <section class="scard warncard">
            <button class="cardhead" aria-expanded={open} onclick={() => toggleGroup("warnings")}>
              <span class="caret" class:open>▸</span>
              <span class="cardname">{m.sidebar_warnings()}</span>
              <span class="cardsum num">{side.warnings.length}</span>
            </button>
            {#if open}
              <div class="cardbody warnings">
                {#each side.warnings as w}
                  <div class="warn">{w}</div>
                {/each}
              </div>
            {/if}
          </section>
        {/if}
      {/if}
    </div>
  {:else}
    <div class="empty">
      <span class="label">{m.sidebar_no_build()}</span>
    </div>
  {/if}

  {#if bd}
    <div bind:this={bdPop} class="bdpop" class:ready={bd.ready} style:top={`${bd.y}px`}>
      <div class="bdhead">
        <span class="label">{m.sidebar_breakdown()}</span>
        {#if bd.pinned}<span class="bdpin" title={m.sidebar_breakdown_pinned()}><Icon name="push-pin" size={12} /></span>{/if}
      </div>
      <div class="bdscroll">
        <BreakdownPanel sections={bd.sections} />
      </div>
    </div>
  {/if}
</aside>

<style>
  .sidebar {
    width: var(--sidebar-w);
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border-right: 1px solid var(--line-0);
    min-height: 0;
  }
  .head {
    padding: 10px 12px 12px;
    border-bottom: 1px solid var(--line-0);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .btn.wide {
    width: 100%;
    margin-top: 2px;
  }
  .field.wide {
    grid-column: 1 / -1;
  }
  .row2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .buildname {
    padding: 0 0 8px;
    margin-bottom: 2px;
    border-bottom: 1px solid var(--line-0);
    display: flex;
    flex-direction: column;
    gap: 4px;
    text-align: left;
    min-width: 0;
  }
  .buildname .label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .bname {
    appearance: none;
    border: 0;
    background: none;
    padding: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    cursor: text;
    text-align: left;
  }
  .bname:hover .bn {
    color: var(--focus);
  }
  .bn {
    font-size: var(--fs-md, 14px);
    font-weight: 600;
    color: var(--fg-0);
    letter-spacing: 0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .unsaved {
    color: var(--warn);
    font-size: 10px;
    flex: 0 0 auto;
  }
  .lorow {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .lorow .select {
    flex: 1;
    min-width: 0;
  }
  .loact {
    appearance: none;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex: 0 0 auto;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    background: var(--bg-2);
    color: var(--fg-1);
    cursor: pointer;
    transition: background 80ms linear, border-color 80ms linear, color 80ms linear;
  }
  .loact:hover:not(:disabled) {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .loact.danger:hover:not(:disabled) {
    color: var(--bad);
  }
  .loact:disabled {
    opacity: var(--fade-off);
    cursor: default;
  }
  .lvl .input {
    width: 100%;
  }
  .lvlrow {
    display: flex;
    align-items: stretch;
    gap: 4px;
  }
  .lvlrow .auto {
    appearance: none;
    flex: none;
    padding: 0 7px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    background: transparent;
    color: var(--fg-3);
    font-family: var(--font-mono);
    font-size: var(--fs-2xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .lvlrow .auto:hover {
    color: var(--fg-0);
  }
  .lvlrow .auto.on {
    color: var(--ok);
  }
  .pts {
    height: 26px;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: var(--fs-sm);
    color: var(--fg-0);
    padding: 0 2px;
  }
  .pts .sep {
    color: var(--fg-3);
  }
  .label2 {
    font-size: var(--fs-xs);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .over {
    color: var(--bad);
  }
  .set1 {
    color: var(--bad);
  }
  .set2 {
    color: var(--ok);
  }
  .stats {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 8px 16px;
    background: var(--bg-0);
    scrollbar-width: none;
    transition: opacity 120ms;
  }
  .stats::-webkit-scrollbar {
    display: none;
  }
  .scard {
    flex: none;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow: clip;
  }
  .cardhead {
    appearance: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 6px 10px;
    border: 0;
    border-bottom: 1px solid var(--line-1);
    background: var(--bg-3);
    color: var(--fg-0);
    text-align: left;
  }
  .cardhead[aria-expanded="false"] {
    border-bottom: 0;
  }
  .cardhead:hover {
    background: var(--bg-hover);
  }
  .caret {
    display: inline-block;
    width: 9px;
    font-size: var(--fs-xs);
    color: var(--fg-2);
    transition: transform 100ms;
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .cardname {
    flex: 1 0 auto;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .cardsum {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: var(--fs-xs);
    color: var(--fg-0);
    white-space: nowrap;
  }
  .cardsum .sep {
    margin: 0 4px;
    color: var(--fg-3);
  }
  .warncard .cardname {
    color: var(--warn);
  }
  .cardbody {
    padding: 5px 10px 6px;
  }
  .stats.busy {
    opacity: 0.6;
  }
  .space {
    height: 7px;
  }
  .shead {
    padding: 8px 0 3px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--fg-2);
  }
  .scenter {
    text-align: center;
    font-size: var(--fs-xs);
    padding: 1px 0;
  }
  .srow {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 10px;
    padding: 1px 0;
    font-size: var(--fs-sm);
    line-height: 17px;
    border-radius: 2px;
  }
  .srow .k {
    color: var(--fg-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .srow .v {
    white-space: nowrap;
    color: var(--fg-0);
  }
  .svalue {
    flex: none;
    display: flex;
    align-items: center;
    gap: 5px;
  }
  .pinmark,
  .bdpin {
    display: grid;
    place-items: center;
    color: var(--focus);
  }
  .srow.hasbd {
    cursor: default;
  }
  .srow.hasbd:hover {
    background: var(--bg-2);
    margin: 0 -6px;
    padding: 1px 6px;
  }
  .bdpop {
    position: fixed;
    left: calc(var(--sidebar-w) + 8px);
    width: min(720px, calc(100vw - var(--sidebar-w) - 24px));
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: color-mix(in srgb, var(--bg-1) 96%, transparent);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-tooltip);
    backdrop-filter: blur(8px);
    z-index: 20;
    pointer-events: none;
    visibility: hidden;
  }
  .bdpop.ready {
    visibility: visible;
  }
  .bdhead {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    border-radius: var(--r-2) var(--r-2) 0 0;
    background: var(--bg-3);
    color: var(--fg-0);
    font-weight: 700;
  }
  .bdscroll {
    padding: 10px 12px;
    overflow-y: auto;
  }
  .warnings {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .warn {
    font-size: var(--fs-xs);
    color: var(--warn);
    line-height: 1.35;
  }
  .empty {
    flex: 1;
    display: grid;
    place-items: center;
  }
</style>
