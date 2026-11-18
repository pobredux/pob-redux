<script lang="ts">
  import { scrollHint } from "$lib/scroll-hint";
  import { tick } from "svelte";
  import { engine, listBuilds, type BuildEntry, type ConfigOption, type ItemInfo } from "$lib/engine.svelte";
  import { build, type ViewId } from "$lib/state/build.svelte";
  import { ui, type Jump } from "$lib/state/ui.svelte";
  import { chat } from "$lib/state/chat.svelte";
  import { appOptions } from "$lib/state/options.svelte";
  import { game, GAMES, GAME_LABEL } from "$lib/state/game.svelte";
  import { loadTree } from "$lib/tree/load";
  import { stripPobText } from "$lib/pobtext";
  import Icon from "./Icon.svelte";
  import Kbd from "./Kbd.svelte";
  import { m } from "$lib/paraglide/messages";

  interface Entry {
    id: string;
    group: string;
    label: string;
    detail?: string;
    /** Searched along with the label, never shown. */
    keywords?: string;
    keys?: string;
    run: () => unknown;
  }

  let query = $state("");
  let active = $state(0);
  let builds = $state<BuildEntry[]>([]);
  let items = $state<ItemInfo[]>([]);
  let configOptions = $state<ConfigOption[]>([]);
  let calcRows = $state<{ label: string; where: string; row: string }[]>([]);
  let passives = $state<{ id: number; name: string; kind: string; stats: string[] }[]>([]);
  let input = $state<HTMLInputElement | null>(null);

  function close() {
    ui.paletteOpen = false;
  }

  function goTo(view: ViewId, jump: Jump | null = null) {
    appOptions.open = false;
    ui.jump = jump;
    build.view = view;
  }

  $effect(() => {
    if (!ui.paletteOpen) return;
    query = "";
    active = 0;
    void tick().then(() => input?.focus());
    let live = true;
    listBuilds().then((b) => live && (builds = b)).catch(() => {});
    if (build.loaded) {
      engine.getItems().then((r) => live && (items = r.items)).catch(() => {});
      engine.listConfigOptions().then((r) => live && (configOptions = r.options)).catch(() => {});
      engine
        .calcSections("player")
        .then((r) => {
          if (!live) return;
          const seen = new Map<string, { label: string; where: string; row: string }>();
          for (const sec of r.sections) {
            for (const sub of sec.subSections) {
              const where = stripPobText(sub.label);
              // A table's columns ("Physical", "Fire", ...) are only named in its unlabelled first row.
              const columns = sub.rows.find((row) => !row.label && row.cells.length > 1)?.cells.map((c) => stripPobText(c.text)) ?? [];
              for (const row of sub.rows) {
                if (!row.label) continue;
                const label = stripPobText(row.label);
                seen.set(`${where}|${label}`, { label, where, row: label });
                if (row.cells.length > 1) {
                  for (const col of columns) if (col) seen.set(`${where}|${label}|${col}`, { label: `${label}: ${col}`, where, row: label });
                }
              }
            }
          }
          calcRows = [...seen.values()];
        })
        .catch(() => {});
      const version = build.tree?.treeVersion;
      if (version) {
        loadTree(version)
          .then((t) => {
            if (!live) return;
            passives = [...t.model.nodes.values()]
              .filter((n) => (n.kind === "notable" || n.kind === "keystone") && !n.hidden && n.name)
              .map((n) => ({ id: n.id, name: n.name, kind: n.kind, stats: n.stats }));
          })
          .catch(() => {});
      }
    }
    return () => {
      live = false;
    };
  });

  const commands = $derived.by((): Entry[] => {
    const g = m.palette_group_commands();
    const list: Entry[] = [];
    if (build.loaded) {
      list.push(
        { id: "save", group: g, label: m.palette_save(), keys: "Mod+S", run: () => build.save() },
        { id: "save-as", group: g, label: m.palette_save_as(), keys: "Mod+Shift+S", run: () => build.saveAs() },
        { id: "undo", group: g, label: m.palette_undo(), keys: "Mod+Z", run: () => build.undo() },
        { id: "redo", group: g, label: m.palette_redo(), keys: "Mod+Y", run: () => build.redo() },
      );
    }
    list.push(
      { id: "new", group: g, label: m.palette_new_build(), run: () => build.newBuild().then((r) => r && (ui.newBuildOpen = true)) },
      { id: "assistant", group: g, label: m.palette_assistant(), keys: "Mod+L", run: () => chat.toggle() },
      { id: "sidebar", group: g, label: m.palette_sidebar(), keys: "Mod+B", run: () => ui.toggleSidebar() },
      { id: "options", group: g, label: m.palette_options(), keys: "Mod+,", run: () => (appOptions.open = true) },
    );
    for (const other of GAMES) {
      if (other !== game.current) list.push({ id: `game-${other}`, group: g, label: m.palette_switch_game({ game: GAME_LABEL[other] }), run: () => game.choose(other) });
    }
    return list;
  });

  const views = $derived.by((): Entry[] => {
    const g = m.palette_group_goto();
    const all: [ViewId, string, string][] = [
      ["import", m.view_builds(), "1"],
      ["overview", m.view_overview(), ""],
      ["tree", m.view_tree(), "2"],
      ["skills", m.view_skills(), "3"],
      ["items", m.view_items(), "4"],
      ["calcs", m.view_calcs(), "5"],
      ["config", m.view_config(), "6"],
      ["notes", m.view_notes(), "7"],
      ["party", m.view_party(), "8"],
      ["optimise", m.view_optimise(), "9"],
      ["compare", m.view_compare(), "0"],
    ];
    return all
      .filter(([id]) => build.loaded || id === "import")
      .map(([id, label, key]) => ({ id: `view-${id}`, group: g, label, keys: key ? `Mod+${key}` : undefined, run: () => goTo(id) }));
  });

  const content = $derived.by((): Entry[] => {
    if (!build.loaded) return [];
    const alloc = new Set(build.tree?.allocatedNodes ?? []);
    const out: Entry[] = [];
    for (const p of passives) {
      out.push({
        id: `node-${p.id}`,
        group: m.palette_group_passives(),
        label: p.name,
        detail: alloc.has(p.id) ? m.palette_allocated() : undefined,
        keywords: p.stats.join(" "),
        run: () => goTo("tree", { view: "tree", node: p.id, name: p.name }),
      });
    }
    for (const it of items) {
      out.push({ id: `item-${it.id}`, group: m.palette_group_items(), label: it.name, detail: it.equippedSlot ?? undefined, run: () => goTo("items", { view: "items", item: it.id }) });
    }
    for (const grp of build.skills?.socketGroups ?? []) {
      const gems = grp.gems.map((gem) => gem.name ?? gem.nameSpec ?? "").filter(Boolean);
      const label = stripPobText(grp.displayLabel ?? grp.label ?? gems[0] ?? "");
      if (!label) continue;
      out.push({
        id: `group-${grp.index}`,
        group: m.palette_group_skills(),
        label,
        detail: grp.slot ?? undefined,
        keywords: gems.join(" "),
        run: () => goTo("skills", { view: "skills", group: grp.index }),
      });
    }
    for (const o of configOptions) {
      const label = stripPobText(o.label ?? o.var).replace(/:$/, "");
      out.push({
        id: `config-${o.var}`,
        group: m.palette_group_config(),
        label,
        detail: o.group ? stripPobText(o.group).replace(/:$/, "") : (o.section ?? undefined),
        run: () => goTo("config", { view: "config", label }),
      });
    }
    for (const { label, where, row } of calcRows) {
      out.push({ id: `calc-${where}-${label}`, group: m.palette_group_calcs(), label, detail: where !== row ? where : undefined, run: () => goTo("calcs", { view: "calcs", label: row }) });
    }
    return out;
  });

  const buildEntries = $derived(
    builds.map((b): Entry => ({
      id: `build-${b.path}`,
      group: m.palette_group_builds(),
      label: b.name,
      detail: [b.ascend_class_name ?? b.class_name, b.level ? `L${b.level}` : null].filter(Boolean).join(" "),
      keywords: b.folder,
      run: () => build.loadFile(b.path),
    })),
  );

  /** Every word must appear; label starts and word starts rank first. */
  function score(e: Entry, words: string[]): number {
    const label = e.label.toLowerCase();
    const rest = `${e.detail ?? ""} ${e.keywords ?? ""}`.toLowerCase();
    let total = 0;
    for (const w of words) {
      const at = label.indexOf(w);
      if (at === 0) total += 4;
      else if (at > 0 && /[\s(\-'/]/.test(label[at - 1])) total += 3;
      else if (at > 0) total += 2;
      else if (rest.includes(w)) total += 0.5;
      else return 0;
    }
    if (label === words.join(" ")) total += 4;
    return total - label.length / 200;
  }

  const groups = $derived.by(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    if (!words.length) {
      return [
        { name: m.palette_group_commands(), entries: commands },
        { name: m.palette_group_goto(), entries: views },
      ].filter((g) => g.entries.length);
    }
    const byGroup = new Map<string, { e: Entry; s: number }[]>();
    for (const e of [...commands, ...views, ...content, ...buildEntries]) {
      const s = score(e, words);
      if (s <= 0) continue;
      let list = byGroup.get(e.group);
      if (!list) byGroup.set(e.group, (list = []));
      list.push({ e, s });
    }
    const ranked = [...byGroup.entries()].map(([name, list]) => {
      list.sort((a, b) => b.s - a.s);
      return { name, best: list[0].s, entries: list.slice(0, 6).map((x) => x.e) };
    });
    ranked.sort((a, b) => b.best - a.best);
    const ask: Entry = { id: "ask", group: m.palette_group_assistant(), label: m.palette_ask({ query: query.trim() }), keys: "Mod+L", run: () => askAssistant() };
    return [...ranked, { name: m.palette_group_assistant(), best: 0, entries: [ask] }];
  });
  const flat = $derived(groups.flatMap((g) => g.entries));

  function askAssistant() {
    chat.input = query.trim();
    if (!chat.open) chat.toggle();
  }

  function run(e: Entry | undefined) {
    if (!e) return;
    close();
    void e.run();
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const n = flat.length;
      if (n) active = (active + (e.key === "ArrowDown" ? 1 : n - 1)) % n;
      document.getElementById(`palette-${active}`)?.scrollIntoView({ block: "nearest" });
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(flat[active]);
    } else if (e.key === "Escape") {
      e.preventDefault();
      close();
    }
  }
</script>

{#if ui.paletteOpen}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
    <div class="palette" role="dialog" aria-modal="true" aria-label={m.palette_title()}>
      <div class="psearch">
        <Icon name="magnifying-glass" size={14} />
        <input
          bind:this={input}
          bind:value={query}
          placeholder={m.palette_placeholder()}
          aria-label={m.palette_title()}
          aria-controls="palette-list"
          aria-activedescendant={flat[active] ? `palette-${active}` : undefined}
          spellcheck="false"
          onkeydown={onKey}
          oninput={() => (active = 0)}
        />
      </div>
      <div class="plist scroll-hint" {@attach scrollHint} id="palette-list" role="listbox" aria-label={m.palette_title()}>
        {#each groups as g (g.name)}
          <div class="ghead" role="presentation">{g.name}</div>
          {#each g.entries as e (e.id)}
            {@const i = flat.indexOf(e)}
            <button
              type="button"
              id={`palette-${i}`}
              role="option"
              aria-selected={i === active}
              class="prow"
              class:hot={i === active}
              tabindex="-1"
              onpointermove={() => (active = i)}
              onclick={() => run(e)}
            >
              <span class="rlabel">{e.label}</span>
              <span class="rdetail">{e.detail ?? ""}</span>
              {#if e.keys}<Kbd keys={e.keys} />{/if}
            </button>
          {/each}
        {:else}
          <div class="pnone">{m.palette_none()}</div>
        {/each}
      </div>
      <div class="pfoot">
        <span><Kbd keys="↑" /><Kbd keys="↓" /> {m.palette_keys_move()}</span>
        <span><Kbd keys="Enter" /> {m.palette_keys_open()}</span>
        <span><Kbd keys="Esc" /> {m.palette_keys_close()}</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: var(--backdrop);
  }
  .palette {
    display: flex;
    flex-direction: column;
    width: min(640px, calc(100vw - 32px));
    max-height: 70vh;
    background: var(--bg-1);
    border: 1px solid var(--line-2);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-modal);
    overflow: hidden;
  }
  .psearch {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 14px;
    border-bottom: 1px solid var(--line-1);
    color: var(--fg-3);
  }
  .psearch input {
    flex: 1;
    height: 44px;
    background: none;
    border: 0;
    outline: none;
    color: var(--fg-0);
    font: inherit;
    font-size: var(--fs-md);
  }
  .psearch input::placeholder {
    color: var(--fg-3);
  }
  .plist {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 4px;
  }
  .ghead {
    padding: 8px 10px 4px;
    color: var(--fg-3);
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }
  .prow {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    min-height: 30px;
    padding: 4px 10px;
    background: none;
    border: 0;
    border-radius: var(--r-1);
    color: var(--fg-1);
    font: inherit;
    font-size: var(--fs-sm);
    text-align: left;
  }
  .prow.hot {
    background: var(--bg-hover);
    color: var(--fg-0);
  }
  .rlabel {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rdetail {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    color: var(--fg-3);
    font-size: var(--fs-xs);
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pnone {
    padding: 14px 10px;
    color: var(--fg-3);
    font-size: var(--fs-sm);
  }
  .pfoot {
    display: flex;
    gap: 16px;
    padding: 6px 12px;
    border-top: 1px solid var(--line-1);
    color: var(--fg-3);
    font-size: var(--fs-xs);
  }
  .pfoot span {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
</style>
