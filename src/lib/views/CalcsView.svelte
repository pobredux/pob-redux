<script lang="ts">
  import { untrack } from "svelte";
  import { engine, type CalcEffect, type CalcRow, type CalcSection, type CalcSkillSelection, type CalcSubSection } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { statusEffectArtUrls } from "$lib/item-art";
  import Icon from "$lib/components/Icon.svelte";
  import PobText from "$lib/components/PobText.svelte";
  import CalcBreakdownWindows from "$lib/components/CalcBreakdownWindows.svelte";
  import { stripPobText } from "$lib/pobtext";
  import { calcBreakdownKey, type CalcBreakdownRef } from "$lib/calc-breakdown";
  import { CHARGE_COLOURS, ROW_COLOURS } from "$lib/calc-colours";
  import { m } from "$lib/paraglide/messages";

  let actor = $state<"player" | "minion">("player");

  const SECTION_OPEN_KEY = "pob-redux:calcs-section-open";
  let sectionOpenOverrides = $state<Record<string, boolean>>({});
  try {
    const stored = JSON.parse(localStorage.getItem(SECTION_OPEN_KEY) ?? "{}");
    if (stored && typeof stored === "object" && !Array.isArray(stored)) sectionOpenOverrides = stored;
  } catch {}

  function sectionIsOpen(section: CalcSection) {
    return sectionOpenOverrides[section.id] ?? sectionIsRelevant(section);
  }

  function toggleSection(section: CalcSection) {
    sectionOpenOverrides = { ...sectionOpenOverrides, [section.id]: !sectionIsOpen(section) };
    try {
      localStorage.setItem(SECTION_OPEN_KEY, JSON.stringify(sectionOpenOverrides));
    } catch {}
  }

  // Which buffs these numbers assume. PoB keeps the sidebar on EFFECTIVE whatever this says.
  let calcMode = $state("EFFECTIVE");
  let calcModes = $state<string[]>(["UNBUFFED", "BUFFED", "COMBAT", "EFFECTIVE"]);
  let calcSkill = $state<CalcSkillSelection>({ group: null, groups: [], activeSkill: null, activeSkills: [], hasMinion: false });
  const BUFF_LABELS = $derived<Record<string, string>>({
    UNBUFFED: m.calcs_buff_unbuffed(),
    BUFFED: m.calcs_buff_buffed(),
    COMBAT: m.calcs_buff_combat(),
    EFFECTIVE: m.calcs_buff_effective(),
  });

  $effect(() => {
    build.rev;
    Promise.all([engine.calcMode(), engine.calcSkill()])
      .then(([mode, skill]) => {
        calcMode = mode.mode;
        calcModes = mode.modes;
        calcSkill = skill;
        if (!skill.hasMinion) actor = "player";
      })
      .catch(() => {});
  });

  function setCalcMode(next: string) {
    calcMode = next;
    build.run(() => engine.calcMode(next));
  }

  function setCalcSkill(patch: { group?: number; activeSkill?: number }) {
    build.run(() => engine.calcSkill(patch)).then((r) => {
      if (r) {
        calcSkill = r;
        if (!r.hasMinion) actor = "player";
      }
    });
  }

  let sections = $state<CalcSection[]>([]);
  let effects = $state<CalcEffect[]>([]);
  let effectArt = $state<Record<string, string>>({});
  let effectRequest = 0;
  let breakdownWindows = $state<ReturnType<typeof CalcBreakdownWindows>>();
  let pinnedBreakdownKeys = $state(new Set<string>());
  const CHARGE_ART = ["Endurance Charge", "Frenzy Charge", "Power Charge"];
  const TOTAL_SECTION_IDS = new Set(["Life", "Mana", "EnergyShield", "Ward", "Armour", "Evasion"]);
  const DEFENCE_RESOURCE_IDS = new Set(["Life", "EnergyShield", "Ward"]);
  const ATTRIBUTE_REQUIREMENTS: Record<string, string> = {
    Strength: "Str. Required",
    Dexterity: "Dex. Required",
    Intelligence: "Int. Required",
    Omniscience: "Omni. Required",
  };
  const ATTRIBUTE_REQUIREMENT_LABELS = new Set(Object.values(ATTRIBUTE_REQUIREMENTS));

  $effect(() => {
    build.rev;
    const a = actor;
    const currentGame = game.current;
    untrack(() => {
      const request = ++effectRequest;
      Promise.all([engine.calcSections(a), engine.calcEffects(a)])
        .then(async ([calc, status]) => {
          if (request !== effectRequest) return;
          sections = calc.sections;
          effects = status.effects;
          effectArt = {};
          const art = await statusEffectArtUrls(currentGame, [...status.effects.map((effect) => effect.artName), ...CHARGE_ART]);
          if (request === effectRequest) effectArt = art;
        })
        .catch(() => {});
    });
  });

  function effectInitials(name: string) {
    return name.split(/\s+/).slice(0, 2).map((word) => word[0]).join("").toUpperCase();
  }

  function cellHasMeaningfulValue(text: string) {
    const plain = stripPobText(text).trim();
    if (!plain) return false;
    const numbers = plain.match(/[-+]?\d[\d,]*(?:\.\d+)?/g);
    if (numbers) return numbers.some((number) => Math.abs(Number(number.replaceAll(",", ""))) > 0.000_001);
    return !["false", "none", "n/a", "-"].includes(plain.toLowerCase());
  }

  function subsectionHasContent(subsection: CalcSubSection) {
    return subsection.rows.some((row) => Boolean(row.label) && row.cells.some((cell) => stripPobText(cell.text).trim().length > 0));
  }

  function sectionIsRelevant(section: CalcSection) {
    const rows = section.subSections.flatMap((sub) => sub.rows).filter((row) => row.label);
    if (rows.length === 0) return false;

    let candidates = rows;
    if (TOTAL_SECTION_IDS.has(section.id)) {
      const totals = rows.filter((row) => stripPobText(row.label ?? "") === "Total");
      if (totals.length > 0) candidates = totals;
    } else if (section.id === "Rage") {
      candidates = rows.filter((row) => stripPobText(row.label ?? "") === "Maximum Rage");
    } else if (section.id === "Charges") {
      candidates = rows.filter((row) => stripPobText(row.label ?? "") === "Current");
    } else if (section.id === "Flasks") {
      candidates = rows.filter((row) => stripPobText(row.label ?? "") !== "Tincture Limit");
    }

    return candidates.some((row) => row.cells.some((cell) => cellHasMeaningfulValue(cell.text)));
  }

  function sortSectionBand(band: CalcSection[], firstId: string, priorityIds: string[] = []) {
    band.sort((a, b) => {
      if (a.id === firstId) return -1;
      if (b.id === firstId) return 1;
      const relevance = Number(sectionIsRelevant(b)) - Number(sectionIsRelevant(a));
      if (relevance !== 0) return relevance;
      const aPriority = priorityIds.indexOf(a.id);
      const bPriority = priorityIds.indexOf(b.id);
      if (aPriority >= 0 || bPriority >= 0) {
        if (aPriority < 0) return 1;
        if (bPriority < 0) return -1;
        return aPriority - bPriority;
      }
      return 0;
    });
  }

  const sectionBands = $derived.by(() => {
    const visible = sections.filter((section) => section.enabled && section.subSections.some(subsectionHasContent));
    const offence = visible.filter((section) => section.group === 1 && section.id !== "DamageTaken");
    const defence = visible.filter((section) => section.group === 3 || section.id === "DamageTaken" || DEFENCE_RESOURCE_IDS.has(section.id));
    const assigned = new Set([...offence, ...defence]);
    const other = visible.filter((section) => !assigned.has(section));

    sortSectionBand(offence, "HitDamage");
    sortSectionBand(defence, "DamageTaken", ["Resist", "Life", "EnergyShield", "Ward"]);
    sortSectionBand(other, "");

    return [
      { id: "offence", label: m.calcs_group_offence(), sections: offence },
      { id: "defence", label: m.calcs_group_defence(), sections: defence },
      { id: "other", label: m.calcs_group_other(), sections: other },
    ].filter((band) => band.sections.length > 0);
  });

  function breakdownKey(sec: CalcSection, sub: number, row: number, col: number) {
    return calcBreakdownKey(breakdownRef(sec, sub, row, col));
  }

  function breakdownRef(sec: CalcSection, sub: number, row: number, col: number): CalcBreakdownRef {
    return { section: sec.index, sub, row, col, actor };
  }

  function isPinned(key: string) {
    return pinnedBreakdownKeys.has(key);
  }

  function showBreakdown(node: HTMLElement, sec: CalcSection, sub: number, row: number, col: number, title: string, pin: boolean) {
    breakdownWindows?.show(node, breakdownRef(sec, sub, row, col), title, pin);
  }

  function leaveBreakdown() {
    breakdownWindows?.leave();
  }

  function isHitDamageHero(sec: CalcSection, row: CalcRow) {
    return sec.id === "HitDamage" && (row.label === "Skill Average Hit" || row.label === "MH Average Hit" || row.label === "Skill DPS");
  }

  function compactHitNumber(value: number) {
    const magnitude = Math.abs(value);
    const compact = (divisor: number, suffix: string) => `${Number((value / divisor).toPrecision(3))}${suffix}`;
    if (magnitude >= 1_000_000_000) return compact(1_000_000_000, "b");
    if (magnitude >= 1_000_000) return compact(1_000_000, "m");
    if (magnitude >= 1_000) return compact(1_000, "k");
    return Math.round(value).toLocaleString("en-US");
  }

  function compactHitDamage(text: string) {
    return stripPobText(text).replace(/-?\d[\d,]*(?:\.\d+)?/g, (match) => compactHitNumber(Number(match.replaceAll(",", ""))));
  }

  function isSkillHitDamageRow(sec: CalcSection, row: CalcRow) {
    return sec.id === "HitDamage" && row.label === "Skill Hit Damage";
  }

  function damageTakenHeroes(section: CalcSection) {
    if (section.id !== "DamageTaken") return [];
    const heroes: { key: string; label: string; value: string; sub: number; row: number; col: number; hasBreakdown: boolean }[] = [];

    const ehpSub = section.subSections.find((sub) => sub.label === 'Effective "Health" Pool');
    const ehpRow = ehpSub?.rows.find((row) => row.label === "Effective Hit Pool");
    const ehpCell = ehpRow?.cells[0];
    if (ehpSub && ehpRow && ehpCell) {
      heroes.push({
        key: "ehp",
        label: m.calcs_ehp(),
        value: ehpCell.text,
        sub: ehpSub.index,
        row: ehpRow.index,
        col: ehpCell.index,
        hasBreakdown: ehpCell.hasBreakdown,
      });
    }

    const maxHitSub = section.subSections.find((sub) => sub.label === "Maximum Hit Taken");
    const typeRow = maxHitSub?.rows.find((row) => !row.label);
    const valueRow = maxHitSub?.rows.find((row) => row.label === "Maximum Hit Taken");
    if (maxHitSub && typeRow && valueRow) {
      for (const cell of valueRow.cells) {
        const type = typeRow.cells.find((header) => header.index === cell.index);
        heroes.push({
          key: `max-hit-${cell.index}`,
          label: `${type?.text.replace(/:$/, "") ?? m.calcs_maximum()} ${m.calcs_hit()}`,
          value: cell.text,
          sub: maxHitSub.index,
          row: valueRow.index,
          col: cell.index,
          hasBreakdown: cell.hasBreakdown,
        });
      }
    }
    return heroes;
  }

  function isPromotedDamageTakenSubsection(section: CalcSection, label: string) {
    return section.id === "DamageTaken" && label === "Maximum Hit Taken";
  }

  function cardHeaderExtra(section: CalcSection, extra: string | null) {
    if (!extra) return extra;
    if (section.id === "Resist") return extra.replace(/[+-]\d+(?=\^7|\/|$)/g, "");
    return extra;
  }

  function calcRowLabel(section: CalcSection, label: string | null) {
    if (!label) return label;
    const colour = ROW_COLOURS[section.id]?.[label];
    return colour ? `${colour}${label}` : label;
  }

  function isAttributeRequirementRow(section: CalcSection, row: CalcRow) {
    return section.id === "Attributes" && ATTRIBUTE_REQUIREMENT_LABELS.has(row.label ?? "");
  }

  function calcCellText(section: CalcSection, subsection: CalcSubSection, row: CalcRow, cellIndex: number, text: string) {
    if (section.id !== "Attributes" || !row.label) return text;
    const requirementLabel = ATTRIBUTE_REQUIREMENTS[row.label];
    if (!requirementLabel) return text;
    const requirement = subsection.rows.find((candidate) => candidate.label === requirementLabel)?.cells.find((cell) => cell.index === cellIndex)?.text;
    return requirement ? `${text} (${m.calcs_required({ value: requirement })})` : text;
  }

  function calcSubsectionLabel(section: CalcSection, label: string) {
    if (section.id !== "Charges") return label;
    return CHARGE_COLOURS[label] ? `${CHARGE_COLOURS[label]}${label}` : label;
  }

  function chargeArtName(section: CalcSection, label: string) {
    if (section.id !== "Charges" || !Object.hasOwn(CHARGE_COLOURS, label)) return null;
    return `${label} Charge`;
  }

  function currentChargeCount(subsection: CalcSubSection) {
    const text = subsection.rows.find((row) => row.label === "Current")?.cells[0]?.text;
    const count = text && Number(stripPobText(text).replaceAll(",", ""));
    return Number.isFinite(count) ? count : 0;
  }

  // Measure cards because the desktop webview lacks CSS masonry rows.
  function masonryCard(node: HTMLElement) {
    let frame = 0;
    const layout = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const grid = node.parentElement;
        if (!grid) return;
        const style = getComputedStyle(grid);
        const rowHeight = Number.parseFloat(style.gridAutoRows);
        const rowGap = Number.parseFloat(style.rowGap);
        if (!Number.isFinite(rowHeight) || !Number.isFinite(rowGap)) return;
        const span = Math.ceil((node.getBoundingClientRect().height + rowGap) / (rowHeight + rowGap));
        node.style.gridRowEnd = `span ${Math.max(1, span)}`;
      });
    };
    const observer = new ResizeObserver(layout);
    observer.observe(node);
    layout();
    return {
      destroy() {
        observer.disconnect();
        cancelAnimationFrame(frame);
      },
    };
  }

</script>

<div class="page">
  <div class="toolbar">
    <label class="fld-inline skill-picker">
      <span class="label">{m.calcs_socket_group()}</span>
      <select
        class="select"
        value={calcSkill.group ?? ""}
        disabled={build.busy > 0 || calcSkill.groups.length === 0}
        onchange={(e) => setCalcSkill({ group: Number((e.target as HTMLSelectElement).value) })}
      >
        {#each calcSkill.groups as group}
          <option value={group.index}>{stripPobText(group.label)}</option>
        {/each}
      </select>
    </label>
    {#if calcSkill.activeSkills.length > 1}
      <label class="fld-inline skill-picker active-skill">
        <span class="label">{m.calcs_active_skill()}</span>
        <select
          class="select"
          value={calcSkill.activeSkill ?? ""}
          disabled={build.busy > 0}
          onchange={(e) => setCalcSkill({ activeSkill: Number((e.target as HTMLSelectElement).value) })}
        >
          {#each calcSkill.activeSkills as skill}
            <option value={skill.index}>{stripPobText(skill.label)}</option>
          {/each}
        </select>
      </label>
    {/if}
    <label class="fld-inline" title={m.calcs_buff_help()}>
      <span class="label">{m.calcs_assuming()}</span>
      <select class="select sm" value={calcMode} disabled={build.busy > 0} onchange={(e) => setCalcMode((e.target as HTMLSelectElement).value)}>
        {#each calcModes as cm}
          <option value={cm}>{BUFF_LABELS[cm] ?? cm}</option>
        {/each}
      </select>
    </label>
    {#if calcSkill.hasMinion}
      <span class="vr"></span>
      <span class="tabs2">
        <button class="t2" class:on={actor === "player"} onclick={() => (actor = "player")}>{m.calcs_player()}</button>
        <button class="t2" class:on={actor === "minion"} onclick={() => (actor = "minion")}>{m.calcs_minion()}</button>
      </span>
    {/if}
  </div>

  <div class="body">
      <div class="cards">
        {#if effects.length > 0}
          <div class="effectbar" role="list">
            {#each effects as effect (`${effect.kind}:${effect.name}`)}
              <div
                class="effect"
                class:debuff={effect.kind === "debuff"}
                role="listitem"
                title={effect.count != null ? `${effect.name} (${effect.count})` : effect.name}
              >
                {#if effectArt[effect.artName]}
                  <img src={effectArt[effect.artName]} alt={effect.name} />
                {:else}
                  <span class="sigil" aria-label={effect.name}>{effectInitials(effect.name)}</span>
                {/if}
                {#if effect.count != null}
                  <span class="effectcount">{effect.count}</span>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
        {#each sectionBands as band (band.id)}
          <div class="sectionband">
            <div class="section-title"><span>{band.label}</span></div>
            <div class="sectiongrid">
              {#each band.sections as sec (sec.index)}
              {@const table = sec.subSections.some((s) => s.rows.some((r) => r.cells.length >= 4))}
              {@const head = sec.subSections[0]}
              {@const open = sectionIsOpen(sec)}
              {@const damageHeroes = damageTakenHeroes(sec)}
              {@const bodySubSections = sec.subSections.filter((sub) => subsectionHasContent(sub) && !isPromotedDamageTakenSubsection(sec, sub.label))}
              {@const headerExtra = cardHeaderExtra(sec, head?.extra ?? null)}
              {#if head}
                <section class="section" class:table use:masonryCard>
                <button class="cardhead" aria-expanded={open} onclick={() => toggleSection(sec)}>
                  <span class="caret" class:open>▸</span>
                  <span class="cardname"><PobText text={head.label} /></span>
                  {#if headerExtra && (!open || sec.id !== "HitDamage")}<span class="extra num"><PobText text={headerExtra} /></span>{/if}
                </button>
                {#if open}
                  <div class="cardbody">
                    {#if sec.id === "HitDamage"}
                      <div class="hit-heroes">
                        {#each sec.subSections as sub (sub.index)}
                          {#each sub.rows.filter((row) => isHitDamageHero(sec, row)) as row (row.index)}
                            {@const cell = row.cells[0]}
                            {#if cell}
                              {@const pinned = isPinned(breakdownKey(sec, sub.index, row.index, cell.index))}
                              <button
                                class="stat-hero"
                                class:link={cell.hasBreakdown}
                                class:pinned
                                aria-pressed={pinned}
                                disabled={!cell.hasBreakdown}
                                onmouseenter={(e) => cell.hasBreakdown && showBreakdown(e.currentTarget, sec, sub.index, row.index, cell.index, stripPobText(row.label ?? ""), false)}
                                onmouseleave={leaveBreakdown}
                                onclick={(e) => showBreakdown(e.currentTarget, sec, sub.index, row.index, cell.index, stripPobText(row.label ?? ""), true)}
                              >
                                {#if pinned}<span class="pinmark hero-pin" title={m.sidebar_breakdown_pinned()}><Icon name="push-pin" size={12} /></span>{/if}
                                <span class="stat-hero-label"><PobText text={row.label ?? ""} /></span>
                                <span class="stat-hero-value num"><PobText text={cell.text} /></span>
                              </button>
                            {/if}
                          {/each}
                        {/each}
                      </div>
                    {/if}
                    {#if damageHeroes.length > 0}
                      <div class="damage-heroes">
                        {#each damageHeroes as hero (hero.key)}
                          {@const pinned = isPinned(breakdownKey(sec, hero.sub, hero.row, hero.col))}
                          <button
                            class="stat-hero"
                            class:link={hero.hasBreakdown}
                            class:pinned
                            aria-pressed={pinned}
                            disabled={!hero.hasBreakdown}
                            onmouseenter={(e) => hero.hasBreakdown && showBreakdown(e.currentTarget, sec, hero.sub, hero.row, hero.col, stripPobText(hero.label), false)}
                            onmouseleave={leaveBreakdown}
                            onclick={(e) => showBreakdown(e.currentTarget, sec, hero.sub, hero.row, hero.col, stripPobText(hero.label), true)}
                          >
                            {#if pinned}<span class="pinmark hero-pin" title={m.sidebar_breakdown_pinned()}><Icon name="push-pin" size={12} /></span>{/if}
                            <span class="stat-hero-label"><PobText text={hero.label} /></span>
                            <span class="stat-hero-value num"><PobText text={hero.value} /></span>
                          </button>
                        {/each}
                      </div>
                    {/if}
                    {#each bodySubSections as sub, si (sub.index)}
                      {@const cols = Math.max(1, ...sub.rows.map((r) => r.cells.length))}
                      {@const chargeArt = chargeArtName(sec, sub.label)}
                      <div class="subsection" class:charge-subsection={chargeArt != null}>
                        {#if sub.index !== head.index}
                          <div class="subhead" class:first={si === 0}>
                            <span class="sublabel"><PobText text={calcSubsectionLabel(sec, sub.label)} /></span>
                            {#if sub.extra && !(sec.id === "DamageTaken" && sub.label === 'Effective "Health" Pool')}
                              <span class="extra num"><PobText text={sub.extra} /></span>
                            {/if}
                          </div>
                        {/if}
                        <div class="rows" class:wide={cols > 1} style:--cols={cols} style:--colw={`${Math.round((sub.colWidth ?? 95) * 0.72)}px`}>
                          {#each sub.rows.filter((row) => !isHitDamageHero(sec, row) && !isAttributeRequirementRow(sec, row)) as row, ri (row.index)}
                            <div class="crow" class:small={row.textSize != null && row.textSize < 16} class:head={ri === 0 && !row.label && row.cells.length > 1}>
                              <span class="rlabel">{#if row.label}<PobText text={calcRowLabel(sec, row.label)} />{/if}</span>
                              {#each row.cells as cell, i (cell.index)}
                                {@const span = cols > 1 && i === row.cells.length - 1 && row.cells.length < cols}
                                {@const pinned = isPinned(breakdownKey(sec, sub.index, row.index, cell.index))}
                                <button
                                  class="cell num"
                                  class:link={cell.hasBreakdown}
                                  class:pinned
                                  class:span
                                  aria-pressed={pinned}
                                  disabled={!cell.hasBreakdown}
                                  title={isSkillHitDamageRow(sec, row) && !cell.hasBreakdown ? stripPobText(cell.text) : undefined}
                                  style:grid-column={span ? `${i + 2} / -1` : null}
                                  onmouseenter={(e) => cell.hasBreakdown && showBreakdown(e.currentTarget, sec, sub.index, row.index, cell.index, stripPobText(`${sub.label} · ${row.label ?? ""}`), false)}
                                  onmouseleave={leaveBreakdown}
                                  onclick={(e) => showBreakdown(e.currentTarget, sec, sub.index, row.index, cell.index, stripPobText(`${sub.label} · ${row.label ?? ""}`), true)}
                                >
                                  <span class="ct">
                                    {#if cell.text}
                                      {#if isSkillHitDamageRow(sec, row)}
                                        {compactHitDamage(cell.text)}
                                      {:else}
                                        <PobText text={calcCellText(sec, sub, row, cell.index, cell.text)} />
                                      {/if}
                                    {/if}
                                  </span>
                                  {#if pinned}<span class="pinmark cell-pin" title={m.sidebar_breakdown_pinned()}><Icon name="push-pin" size={10} /></span>{/if}
                                </button>
                              {/each}
                            </div>
                          {/each}
                        </div>
                        {#if chargeArt}
                          {@const chargeCount = currentChargeCount(sub)}
                          <div class="effect charge-effect" title={`${sub.label} Charges (${chargeCount})`}>
                            {#if effectArt[chargeArt]}
                              <img src={effectArt[chargeArt]} alt={`${sub.label} Charge`} />
                            {:else}
                              <span class="sigil" aria-label={`${sub.label} Charge`}>{effectInitials(sub.label)}</span>
                            {/if}
                            <span class="effectcount">{chargeCount}</span>
                          </div>
                        {/if}
                      </div>
                    {/each}
                  </div>
                {/if}
                </section>
              {/if}
              {/each}
            </div>
          </div>
        {/each}
      </div>
      <CalcBreakdownWindows
        bind:this={breakdownWindows}
        revision={build.rev}
        onpinnedchange={(keys) => (pinnedBreakdownKeys = keys)}
      />
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
    gap: 12px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-1);
    overflow-x: auto;
  }
  .fld-inline {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    flex: none;
  }
  .skill-picker .select {
    width: clamp(150px, 18vw, 260px);
  }
  .skill-picker.active-skill .select {
    width: clamp(130px, 16vw, 230px);
  }
  .tabs2 {
    display: inline-flex;
    gap: 2px;
  }
  .t2 {
    appearance: none;
    border: 0;
    background: none;
    color: var(--fg-2);
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: 3px;
  }
  .t2.on {
    color: var(--fg-0);
    background: var(--bg-active);
  }
  .vr {
    width: 1px;
    height: 16px;
    background: var(--line-1);
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
  }
  .cards {
    flex: 1;
    min-width: 0;
    overflow: auto;
    container-type: inline-size;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 14px;
  }
  .sectiongrid {
    min-width: 0;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    grid-auto-rows: 1px;
    grid-auto-flow: dense;
    column-gap: 14px;
    row-gap: 8px;
    align-items: start;
  }
  .sectionband {
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .sectionband + .sectionband {
    margin-top: 8px;
  }
  .section-title {
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--fg-2);
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.09em;
    line-height: 1;
  }
  .section-title::before,
  .section-title::after {
    content: "";
    height: 1px;
    flex: 1;
    background: var(--line-2);
  }
  .section-title::before {
    flex: 0 0 16px;
  }
  .effectbar {
    flex: none;
    min-width: 0;
    min-height: 68px;
    display: flex;
    align-items: flex-start;
    gap: 6px;
    /* Count badges hang 8px below the icons; reserve that space plus breathing room. */
    padding: 2px 2px 18px;
    overflow-x: auto;
    scrollbar-width: thin;
  }
  .effect {
    --effect-accent: var(--ok);
    --effect-art-border: var(--ok);
    position: relative;
    flex: 0 0 48px;
    width: 48px;
    height: 48px;
    padding: 4px;
    border: 1px solid var(--line-2);
    border-radius: 3px;
    background: color-mix(in srgb, var(--effect-accent) 14%, var(--bg-3));
    box-shadow:
      inset 0 0 0 2px var(--bg-2),
      inset 0 0 7px var(--bg-0);
  }
  .effect.debuff {
    --effect-accent: var(--bad);
    --effect-art-border: var(--bad);
  }
  .effect img,
  .sigil {
    width: 100%;
    height: 100%;
    display: grid;
    place-items: center;
    border: 1px solid var(--effect-art-border);
    background: var(--bg-0);
  }
  .effect img {
    display: block;
    object-fit: cover;
  }
  .sigil {
    color: var(--fg-0);
    font-size: var(--fs-xs);
    font-weight: 700;
    letter-spacing: 0.03em;
  }
  .effectcount {
    position: absolute;
    left: 50%;
    bottom: -8px;
    min-width: 18px;
    padding: 0 4px;
    transform: translateX(-50%);
    border: 1px solid var(--line-2);
    border-radius: 2px;
    background: var(--bg-0);
    color: var(--fg-0);
    font-size: 11px;
    font-weight: 700;
    line-height: 14px;
    text-align: center;
    box-shadow: 0 1px 2px var(--bg-0);
  }
  .section {
    display: block;
    min-width: 0;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    overflow: clip;
  }
  .section.table {
    grid-column: span 2;
  }
  @container (width < 720px) {
    .section.table {
      grid-column: auto;
    }
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
  .extra {
    font-size: var(--fs-xs);
    color: var(--fg-0);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cardbody {
    padding: 0;
  }
  .hit-heroes,
  .damage-heroes {
    display: grid;
    gap: 8px;
    padding: 10px 12px 4px;
  }
  .hit-heroes {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
  .hit-heroes .stat-hero:only-child {
    grid-column: 1 / -1;
  }
  .damage-heroes {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }
  @container (width < 900px) {
    .damage-heroes {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }
  }
  @container (width < 520px) {
    .damage-heroes {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
  .stat-hero {
    appearance: none;
    position: relative;
    min-width: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 10px 12px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    background: var(--bg-2);
    color: var(--fg-0);
    font: inherit;
    text-align: center;
  }
  .stat-hero-label {
    color: var(--fg-2);
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .stat-hero.pinned .stat-hero-label {
    max-width: calc(100% - 18px);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transform: translateX(-7px);
  }
  .stat-hero-value {
    max-width: 100%;
    font-size: 22px;
    font-weight: 700;
    line-height: 1.15;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .stat-hero.link {
    cursor: pointer;
  }
  .stat-hero.link:hover {
    border-color: var(--line-2);
    background: var(--bg-hover);
  }
  .subhead {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
    padding: 8px 12px 4px;
    margin-top: 6px;
    border-top: 1px solid var(--line-0);
  }
  .subhead.first {
    border-top: 0;
  }
  .subsection {
    min-width: 0;
  }
  .charge-subsection {
    position: relative;
  }
  .charge-subsection .rows {
    padding-right: 82px;
  }
  .charge-effect {
    position: absolute;
    top: 55%;
    right: 14px;
    transform: translateY(-50%);
    border-color: var(--line-1);
    background: var(--bg-3);
    box-shadow:
      inset 0 0 0 2px var(--bg-2),
      inset 0 0 7px var(--bg-0);
  }
  .sublabel {
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--fg-2);
  }
  /* One grid per subsection, so a table's columns line up across its rows as PoB draws them. */
  .rows {
    display: grid;
    grid-template-columns: minmax(130px, max-content) minmax(0, 1fr);
    column-gap: 8px;
    align-items: baseline;
    padding: 6px 12px 8px;
    font-size: var(--fs-sm);
    line-height: 1.5;
  }
  .rows.wide {
    grid-template-columns: minmax(130px, 1.35fr) repeat(var(--cols), minmax(max-content, 1fr));
    column-gap: 0;
  }
  .crow {
    display: contents;
  }
  .crow.small {
    font-size: var(--fs-xs);
  }
  .crow.head > * {
    color: var(--fg-2);
    font-size: var(--fs-xs);
    padding-bottom: 4px;
    border-bottom: 1px solid var(--line-0);
  }
  .crow.head > .rlabel {
    border-bottom: 0;
  }
  .wide .crow:nth-child(even) > * {
    background: var(--bg-2);
  }
  .rlabel {
    grid-column: 1;
    padding: 2px 0;
    color: var(--fg-2);
  }
  .wide .rlabel {
    padding: 2px 8px 2px 6px;
  }
  .cell {
    appearance: none;
    position: relative;
    border: 0;
    background: none;
    padding: 2px 4px;
    color: var(--fg-0);
    font-size: inherit;
    line-height: inherit;
    text-align: left;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .rows:not(.wide) .cell {
    white-space: normal;
    overflow: visible;
  }
  .wide .cell {
    padding: 2px 10px 2px 8px;
    text-align: right;
    overflow: visible;
  }
  .wide .cell.span {
    text-align: left;
  }
  /* Keep PoB's value-column floor inside the cell. */
  .wide .ct {
    display: inline-block;
    min-width: var(--colw);
  }
  .cell.link {
    cursor: pointer;
    border-radius: 3px;
  }
  .cell.link:hover {
    color: var(--focus);
    background: var(--bg-hover);
  }
  .pinmark {
    display: grid;
    place-items: center;
    color: var(--focus);
  }
  .hero-pin {
    position: absolute;
    top: 6px;
    right: 6px;
  }
  .cell-pin {
    position: absolute;
    top: 50%;
    right: 5px;
    transform: translateY(-50%);
  }
  /* Sit inside the 10px right padding so pinned numbers stay on their column. */
  .wide .cell-pin {
    right: 0;
  }
</style>
