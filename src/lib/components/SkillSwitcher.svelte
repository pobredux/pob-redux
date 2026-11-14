<script lang="ts">
  import { engine, type SocketGroup } from "$lib/engine.svelte";
  import { build } from "$lib/state/build.svelte";
  import { game } from "$lib/state/game.svelte";
  import { skillArtUrls } from "$lib/item-art";
  import { stripPobText } from "$lib/pobtext";
  import Switcher, { type SwitcherSection } from "./Switcher.svelte";
  import MinionLibrary from "./MinionLibrary.svelte";
  import { m } from "$lib/paraglide/messages";

  const GRANT = { mechanic: "◈", node: "✦", item: "⚔" } as const;

  const info = $derived(build.info);
  const groups = $derived(build.skills?.socketGroups ?? []);
  const skillOf = (g: SocketGroup) => g.skills.find((s) => s.index === (g.mainActiveSkill ?? 1)) ?? g.skills[0];
  const mainGroup = $derived(groups.find((g) => g.index === info?.mainSocketGroup));
  const mainSkill = $derived(mainGroup && skillOf(mainGroup));
  const groupLabel = (g: SocketGroup) =>
    stripPobText(g.displayLabel ?? g.label ?? m.sidebar_group_fallback({ index: g.index })) + (g.duplicateOf ? m.sidebar_group_item_copy({ source: g.duplicateOf }) : "");
  const bare = (label: string) => label.replace(/\s*\(.*\)\s*$/, "");
  const artNames = (g: SocketGroup) =>
    [skillOf(g)?.name, ...g.skills.map((s) => s.name), ...g.gems.filter((x) => !x.support).map((x) => x.nameSpec ?? x.name), bare(groupLabel(g))].filter(
      (x): x is string => !!x,
    );

  let art = $state<Record<string, string>>({});
  const artFor = (names: (string | null | undefined)[]) => names.map((n) => (n ? art[n] : undefined)).find(Boolean);
  $effect(() => {
    const names = [...groups.flatMap(artNames), ...(mainSkill?.minions?.map((x) => x.name) ?? []), ...(mainSkill?.minionSkills ?? [])];
    const current = game.current;
    let live = true;
    skillArtUrls(current, [...new Set(names)])
      .then((urls) => live && (art = urls))
      .catch(() => {});
    return () => {
      live = false;
    };
  });

  function patch(options: Parameters<typeof engine.setMainSkillOptions>[1]) {
    if (mainGroup) build.run(() => engine.setMainSkillOptions(mainGroup.index, options));
  }

  const statSet = $derived(mainSkill?.statSets?.[(mainSkill.statSet ?? 1) - 1]);
  const part = $derived(mainSkill?.parts?.[(mainSkill.part ?? 1) - 1]?.name);
  const title = $derived(mainSkill?.name ?? (mainGroup ? groupLabel(mainGroup) : m.sidebar_no_skills()));
  const detail = $derived(
    [mainGroup && bare(groupLabel(mainGroup)), statSet, part].filter((x): x is string => !!x && x !== title).join(" · ") || m.sidebar_main_skill(),
  );

  const skillSections = $derived.by(() => {
    const out: SwitcherSection[] = [
      {
        key: "groups",
        title: m.sidebar_switch_skills(),
        options: groups.map((g) => ({
          key: String(g.index),
          label: groupLabel(g),
          art: artFor(artNames(g)),
          checked: g.index === info?.mainSocketGroup,
          dim: !g.enabled,
          hint: g.grantedBy ? GRANT[g.grantedBy.kind] : undefined,
          hintTitle: g.grantedBy?.source,
          onpick: () => build.setMainSkill(g.index),
        })),
      },
    ];
    const group = mainGroup;
    const skill = mainSkill;
    if (group && group.skills.length > 1) {
      out.push({
        key: "active",
        title: m.sidebar_active_skill(),
        options: group.skills.map((s) => ({
          key: String(s.index),
          label: s.name,
          art: art[s.name],
          checked: s.index === (group.mainActiveSkill ?? 1),
          onpick: () => patch({ mainActiveSkill: s.index }),
        })),
      });
    }
    if (skill?.statSets?.length) {
      out.push({
        key: "statset",
        title: m.sidebar_stat_set(),
        options: skill.statSets.map((label, i) => ({
          key: String(i),
          label,
          number: i + 1,
          checked: i + 1 === (skill.statSet ?? 1),
          onpick: () => patch({ statSet: i + 1 }),
        })),
      });
    }
    if (skill?.parts?.length) {
      out.push({
        key: "part",
        title: m.sidebar_skill_part(),
        options: skill.parts.map((p, i) => ({
          key: String(i),
          label: p.name,
          number: i + 1,
          checked: i + 1 === (skill.part ?? 1),
          onpick: () => patch({ part: i + 1 }),
        })),
      });
    }
    return out;
  });

  const minion = $derived(mainSkill?.minions?.find((x) => x.id === (mainSkill?.minion ?? mainSkill?.minions?.[0]?.id)));
  const minionSkill = $derived(mainSkill?.minionSkills?.[(mainSkill.minionSkill ?? 1) - 1]);
  const minionSet = $derived(mainSkill?.minionItemSets?.find((x) => x.id === (mainSkill?.minionItemSet ?? mainSkill?.minionItemSets?.[0]?.id)));
  const hasMinion = $derived(!!(mainSkill?.minions?.length || mainSkill?.minionSkills?.length || mainSkill?.minionItemSets?.length || mainSkill?.minionLibrary));

  const minionSections = $derived.by(() => {
    const skill = mainSkill;
    const out: SwitcherSection[] = [];
    if (!skill) return out;
    if (skill.minions?.length) {
      out.push({
        key: "minion",
        title: m.sidebar_minion(),
        options: skill.minions.map((x) => ({
          key: x.id,
          label: x.name,
          art: art[x.name],
          checked: x.id === minion?.id,
          onpick: () => patch({ minionId: x.id }),
        })),
      });
    }
    if (skill.minionSkills?.length) {
      out.push({
        key: "minionskill",
        title: m.skills_minion_skill(),
        options: skill.minionSkills.map((name, i) => ({
          key: String(i),
          label: name,
          art: artFor([name, minion?.name]),
          number: i + 1,
          checked: i + 1 === (skill.minionSkill ?? 1),
          onpick: () => patch({ minionSkill: i + 1 }),
        })),
      });
    }
    if (skill.minionItemSets?.length) {
      out.push({
        key: "itemset",
        title: m.skills_minion_item_set(),
        options: skill.minionItemSets.map((set, i) => ({
          key: String(set.id),
          label: set.title,
          number: i + 1,
          checked: set.id === minionSet?.id,
          onpick: () => patch({ minionItemSet: set.id }),
        })),
      });
    }
    if (skill.minionLibrary) {
      const beast = skill.minionLibrary === "beast";
      out.push({
        key: "library",
        options: [
          {
            key: "library",
            label: beast ? m.sidebar_manage_beasts() : m.sidebar_manage_spectres(),
            title: beast ? m.sidebar_manage_beasts_title() : m.sidebar_manage_spectres_title(),
            action: true,
            onpick: () => (libraryOpen = true),
          },
        ],
      });
    }
    return out;
  });
  const minionTitle = $derived(minionSkill ?? minion?.name ?? minionSet?.title ?? m.sidebar_minion());
  const minionDetail = $derived([minion?.name, minion ? minionSet?.title : null].filter((x): x is string => !!x && x !== minionTitle).join(" · ") || m.sidebar_minion());

  let libraryOpen = $state(false);
</script>

<div class="switchers">
  <Switcher
    {title}
    {detail}
    art={artFor([mainSkill?.name, ...(mainGroup ? artNames(mainGroup) : [])])}
    label={m.sidebar_switch_title()}
    sections={skillSections}
    disabled={groups.length === 0}
  />
  {#if hasMinion}
    <Switcher title={minionTitle} detail={minionDetail} art={artFor([minionSkill, minion?.name])} label={m.sidebar_minion_title()} sections={minionSections} />
  {/if}
</div>

{#if libraryOpen}
  <MinionLibrary kind={mainSkill?.minionLibrary ?? "spectre"} onclose={() => (libraryOpen = false)} />
{/if}

<style>
  .switchers {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
</style>
