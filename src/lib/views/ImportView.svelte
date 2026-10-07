<script lang="ts">
  import { onMount } from "svelte";
  import { readText, writeText } from "@tauri-apps/plugin-clipboard-manager";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import {
    engine,
    listBuilds,
    setBuildsDir,
    listBuildFolders,
    renameBuild,
    moveBuild,
    deleteBuild,
    createBuildFolder,
    deleteBuildFolder,
    renameBuildFolder,
    fetchBuildCode,
    isMobalyticsLink,
    resolveMobalytics,
    isMaxrollGuideLink,
    resolveMaxroll,
    characterList,
    characterData,
    ninjaCharacters,
    ninjaCharacterCode,
    type GameCharacter,
    type ReimportOptions,
    saveGameBuildFiles,
    listGameBuilds,
    setGameBuildMeta,
    shareBuildCode,
    readTextFile,
    writeTextFile,
    SHARE_SITES,
    type ShareSite,
    type AppPaths,
    type PlannerMode,
    type BuildEntry,
    type GameBuildList,
    type MobalyticsBuild,
    type MobalyticsVariant,
    type MaxrollGuide,
    type MaxrollPobLink,
  } from "$lib/engine.svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { build, autosaveKey } from "$lib/state/build.svelte";
  import { ui } from "$lib/state/ui.svelte";
  import { app } from "$lib/state/app.svelte";
  import { planner, AUTHOR_KEY } from "$lib/state/planner.svelte";
  import { game } from "$lib/state/game.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import ClassIcon from "$lib/components/ClassIcon.svelte";
  import Kbd from "$lib/components/Kbd.svelte";
  import { modKey } from "$lib/keys";
  import { m } from "$lib/paraglide/messages";

  let { paths }: { paths: AppPaths | null } = $props();

  let code = $state("");
  let builds = $state<BuildEntry[]>([]);
  let folders = $state<string[]>([]);
  let filter = $state("");
  let filterEl = $state<HTMLInputElement>();
  function onFindKey(e: KeyboardEvent) {
    if (!modKey(e) || e.shiftKey || e.altKey || e.key.toLowerCase() !== "f" || document.querySelector('[aria-modal="true"]')) return;
    e.preventDefault();
    filterEl?.focus();
    filterEl?.select();
  }
  let sort = $state<"modified" | "name" | "level" | "class">("modified");
  let flash = $state<string | null>(null);
  let fetching = $state(false);

  // The name written as `author` into exported .build files; remembered across builds.
  let author = $state("");
  try {
    author = localStorage.getItem(AUTHOR_KEY) ?? "";
  } catch {}
  function commitAuthor() {
    author = author.trim();
    try {
      localStorage.setItem(AUTHOR_KEY, author);
    } catch {}
  }

  // Share link (pobb.in and friends), as PoB's own Share button.
  const SITE_KEY = "pob-redux:share-site";
  let shareSite = $state<ShareSite>("pobb.in");
  try {
    const saved = localStorage.getItem(SITE_KEY);
    if (saved && (SHARE_SITES as readonly string[]).includes(saved)) shareSite = saved as ShareSite;
  } catch {}
  let sharing = $state(false);
  let shareUrl = $state<string | null>(null);

  // Editing the author of a group of game builds (one input per header),
  // and the name or author of one file (one input per row).
  let editAuthor = $state<string | null>(null);
  let authorDraft = $state("");
  let gbEdit = $state<{ path: string; field: "name" | "author" } | null>(null);
  let gbDraft = $state("");

  // The open build's name, edited in place; Enter or blur commits.
  let nameDraft = $state<string | null>(null);
  function commitBuildName() {
    const name = nameDraft?.trim() ?? "";
    nameDraft = null;
    if (name && name !== build.info?.name) build.rename(name).then(() => refresh());
  }

  let renaming = $state<string | null>(null);
  let renameDraft = $state("");
  let moving = $state<string | null>(null);
  let confirmDelete = $state<string | null>(null);
  let movingNew = $state<string | null>(null);
  let folderDraft = $state("");

  // recent builds (paths, most recent first)
  const RECENT_KEY = "pob-redux:recent";
  let recent = $state<string[]>([]);
  try {
    recent = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
  } catch {
    recent = [];
  }
  function noteRecent(path: string) {
    recent = [path, ...recent.filter((p) => p !== path)].slice(0, 8);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
    } catch {}
  }

  let autosave = $state<{ name: string; file: string | null; at: number; xml: string } | null>(null);
  try {
    autosave = JSON.parse(localStorage.getItem(autosaveKey()) ?? "null");
  } catch {
    autosave = null;
  }

  const shown = $derived.by(() => {
    const q = filter.trim().toLowerCase();
    const list = builds.filter((b) => !q || `${b.folder}/${b.name} ${b.class_name ?? ""} ${b.ascend_class_name ?? ""}`.toLowerCase().includes(q));
    const cmp: Record<typeof sort, (a: BuildEntry, b: BuildEntry) => number> = {
      modified: (a, b) => b.modified - a.modified,
      name: (a, b) => a.name.localeCompare(b.name),
      level: (a, b) => (b.level ?? 0) - (a.level ?? 0),
      class: (a, b) => (a.class_name ?? "").localeCompare(b.class_name ?? "") || a.name.localeCompare(b.name),
    };
    return [...list].sort(cmp[sort]);
  });

  const grouped = $derived.by(() => {
    const groups = new Map<string, BuildEntry[]>();
    for (const f of folders) groups.set(f, []);
    for (const b of shown) {
      if (!groups.has(b.folder)) groups.set(b.folder, []);
      groups.get(b.folder)!.push(b);
    }
    return [...groups.entries()]
      .filter(([f, items]) => items.length > 0 || (!filter.trim() && f !== ""))
      .sort(([a], [b]) => (a === "" ? -1 : b === "" ? 1 : a.localeCompare(b)));
  });

  const ascendByKey = $derived.by(() => {
    const map = new Map<string, { cls: string; asc: string }>();
    for (const c of build.classes) {
      for (const a of c.ascendancies) {
        const v = { cls: c.name, asc: a.name };
        map.set(a.name, v);
        if (a.internalId) map.set(a.internalId, v);
      }
    }
    return map;
  });

  // The open build's version keeps its tree in loadTree's one-entry cache.
  const artVersion = $derived(build.tree?.treeVersion ?? build.meta?.latestTreeVersion ?? null);

  // As PoB colours classes: by their strongest base attributes.
  const ATTR_COLOUR: Record<string, string> = {
    "100": "var(--c-life)",
    "010": "var(--ok)",
    "001": "var(--c-mana)",
    "110": "var(--c-rare)",
    "101": "var(--c-chaos)",
    "011": "var(--c-es)",
    "111": "color-mix(in oklab, var(--c-normal) 45%, var(--c-life))",
  };
  // A wider hue step would cross into a neighbouring class's hue, so siblings also step in lightness.
  const ASCENDANCY_HUE_STEP = 12;
  const ASCENDANCY_LIGHTNESS_STEP = 0.07;
  const classColour = $derived.by(() => {
    const map = new Map<string, string>();
    for (const c of build.classes) {
      const attrs = [c.str ?? 0, c.dex ?? 0, c.int ?? 0];
      const top = Math.max(...attrs);
      const colour = ATTR_COLOUR[attrs.map((v) => (v === top ? 1 : 0)).join("")];
      if (!top || !colour) continue;
      map.set(c.name, colour);
      c.ascendancies.forEach((a, i, all) => {
        const t = all.length > 1 ? (2 * i) / (all.length - 1) - 1 : 0;
        const shade = `oklch(from ${colour} calc(l - ${ASCENDANCY_LIGHTNESS_STEP * t}) c calc(h + ${ASCENDANCY_HUE_STEP * t}))`;
        map.set(a.name, shade);
        if (a.internalId) map.set(a.internalId, shade);
      });
    }
    return map;
  });
  const buildColour = (cls: string | null | undefined, asc: string | null | undefined) =>
    classColour.get(asc ?? "") ?? classColour.get(cls ?? "");

  const recentEntries = $derived(recent.map((p) => builds.find((b) => b.path === p)).filter((b): b is BuildEntry => !!b));

  // GGG Build Planner (*.build) files
  let gameBuilds = $state<GameBuildList | null>(null);
  let plannerDir = $state("");
  let gbCollapsed = $state<Set<string>>(new Set());
  try {
    plannerDir = localStorage.getItem("pob-redux:planner-dir") ?? "";
    gbCollapsed = new Set(JSON.parse(localStorage.getItem("pob-redux:gb-collapsed") ?? "[]"));
  } catch {}

  const gameBuildGroups = $derived.by(() => {
    const groups = new Map<string, GameBuildList["builds"]>();
    for (const b of gameBuilds?.builds ?? []) {
      const author = b.author ?? m.import_unknown_author();
      if (!groups.has(author)) groups.set(author, []);
      groups.get(author)!.push(b);
    }
    return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b, undefined, { sensitivity: "base" }));
  });

  function toggleAuthor(author: string) {
    const s = new Set(gbCollapsed);
    s.has(author) ? s.delete(author) : s.add(author);
    gbCollapsed = s;
    try {
      localStorage.setItem("pob-redux:gb-collapsed", JSON.stringify([...s]));
    } catch {}
  }

  let folderCollapsed = $state<Set<string>>(new Set());
  try {
    folderCollapsed = new Set(JSON.parse(localStorage.getItem("pob-redux:folders-collapsed") ?? "[]"));
  } catch {}
  const folderOpen = (folder: string) => !!filter.trim() || !folderCollapsed.has(folder);

  function toggleFolder(folder: string) {
    const s = new Set(folderCollapsed);
    s.has(folder) ? s.delete(folder) : s.add(folder);
    folderCollapsed = s;
    try {
      localStorage.setItem("pob-redux:folders-collapsed", JSON.stringify([...s]));
    } catch {}
  }

  let newFolder = $state<string | null>(null);
  let confirmFolder = $state<string | null>(null);

  /** Builds in a folder ignoring the filter, which is what emptiness means here. */
  function folderCount(folder: string) {
    return builds.filter((b) => b.folder === folder).length;
  }

  async function commitNewFolder() {
    const name = (newFolder ?? "").trim();
    if (!name) {
      newFolder = null;
      return;
    }
    newFolder = null;
    try {
      await createBuildFolder(name);
      say(m.import_folder_created({ name }));
      await refresh();
    } catch (e) {
      build.error = String(e);
    }
  }

  let renamingFolder = $state<string | null>(null);
  let folderRenameDraft = $state("");

  const inFolder = (rel: string, folder: string) => rel === folder || rel.startsWith(`${folder}/`);

  async function commitRenameFolder(folder: string) {
    const name = folderRenameDraft.trim();
    renamingFolder = null;
    if (!name || name === folder.split("/").pop()) return;
    const before = builds.filter((b) => inFolder(b.folder, folder));
    try {
      const next = await renameBuildFolder(folder, name);
      await refresh();
      const moved = new Map<string, string>();
      for (const b of before) {
        const now = builds.find((x) => x.folder === next + b.folder.slice(folder.length) && x.name === b.name);
        if (now) moved.set(b.path, now.path);
      }
      recent = recent.map((p) => moved.get(p) ?? p);
      folderCollapsed = new Set([...folderCollapsed].map((f) => (inFolder(f, folder) ? next + f.slice(folder.length) : f)));
      try {
        localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
        localStorage.setItem("pob-redux:folders-collapsed", JSON.stringify([...folderCollapsed]));
      } catch {}
      const open = build.info?.file;
      if (open && moved.has(open)) await build.loadFile(moved.get(open)!);
      say(m.import_folder_renamed({ name: next }));
    } catch (e) {
      build.error = String(e);
    }
  }

  async function commitDeleteFolder(folder: string) {
    confirmFolder = null;
    try {
      await deleteBuildFolder(folder);
      say(m.import_folder_deleted({ folder }));
      await refresh();
    } catch (e) {
      build.error = String(e);
    }
  }

  async function refresh() {
    builds = await listBuilds().catch(() => []);
    folders = await listBuildFolders().catch(() => []);
    // The game's Build Planner is a PoE2 feature.
    gameBuilds = game.isPoe2 ? await listGameBuilds(plannerDir || undefined).catch(() => null) : null;
  }
  onMount(refresh);

  function setPlannerDir(dir: string) {
    plannerDir = dir.trim();
    try {
      localStorage.setItem("pob-redux:planner-dir", plannerDir);
    } catch {}
    refresh();
  }

  async function browsePlannerDir() {
    const dir = await open({ directory: true, defaultPath: gameBuilds?.dir || undefined, title: m.import_browse_title() }).catch(() => null);
    if (typeof dir === "string") setPlannerDir(dir);
  }

  async function changeBuildsDir(dir: string | null) {
    try {
      await setBuildsDir(dir);
      await app.refreshPaths();
      await refresh();
    } catch (e) {
      say(String(e));
    }
  }

  async function browseBuildsDir() {
    const dir = await open({ directory: true, defaultPath: paths?.builds_dir, title: m.import_builds_browse_title() }).catch(() => null);
    if (typeof dir === "string") await changeBuildsDir(dir);
  }

  function reveal(path: string) {
    revealItemInDir(path).catch((e) => say(String(e)));
  }

  const rootOnly = $derived(grouped.length === 1 && grouped[0][0] === "");

  async function importGameBuildFile(path: string, name: string) {
    try {
      const json = await readTextFile(path);
      await importGameBuildJson(json, name);
    } catch (e) {
      build.error = String(e);
      say(m.import_failed({ error: String(e) }));
    }
  }

  async function importGameBuildJson(json: string, name: string) {
    try {
      const r = await build.run(() => engine.importGameBuild(json, name));
      if (r) {
        const issues = [...r.warnings, ...r.missingPassives.map((p) => m.import_gb_unknown_passive({ name: p })), ...r.missingSkills.map((s) => m.import_gb_unknown_gem({ name: s }))];
        const gear = r.gearItems > 0 ? m.import_gb_gear({ count: r.gearItems }) : r.gearHints > 0 ? "" : m.import_gb_no_gear();
        const hints = r.gearHints > 0 ? m.import_gb_hints({ count: r.gearHints }) : "";
        const passives = r.allocated < r.requested ? m.import_gb_passives_partial({ allocated: r.allocated, requested: r.requested }) : m.import_gb_passives({ count: r.allocated });
        build.say(m.import_gb_summary({ passives, groups: r.skillGroups, gear, hints, issues: issues.length ? m.import_gb_issues({ count: issues.length }) : "" }));
        build.view = "tree";
      } else if (build.error) {
        say(m.import_failed({ error: build.error }));
      }
    } catch (e) {
      build.error = String(e);
      say(m.import_failed({ error: String(e) }));
    }
  }

  async function commitGroupAuthor(items: GameBuildList["builds"]) {
    const name = authorDraft.trim();
    editAuthor = null;
    try {
      for (const gb of items) await setGameBuildMeta(gb.path, { author: name });
      const n = m.import_files_count({ count: items.length });
      say(name ? m.import_author_set_on({ name, count: n }) : m.import_author_cleared_on({ count: n }));
    } catch (e) {
      build.error = String(e);
    }
    refresh();
  }

  async function commitGbEdit(gb: GameBuildList["builds"][number]) {
    const edit = gbEdit;
    const value = gbDraft.trim();
    gbEdit = null;
    if (!edit) return;
    if (edit.field === "name" && (!value || value === gb.name)) return;
    if (edit.field === "author" && value === (gb.author ?? "")) return;
    try {
      await setGameBuildMeta(gb.path, edit.field === "name" ? { name: value } : { author: value });
      say(edit.field === "name" ? m.import_renamed_to({ name: value }) : value ? m.import_author_set({ name: value }) : m.import_author_cleared());
    } catch (e) {
      build.error = String(e);
    }
    refresh();
  }

  let plannerOpen = $state(false);
  let plannerLoadouts = $state(0);
  let plannerMode = $state<PlannerMode>("current");
  let plannerAuto = $state(true);
  let plannerBusy = $state(false);
  const plannerLink = $derived(planner.linkFor(build.info?.file));
  const plannerModes = $derived<[PlannerMode, string, string][]>([
    ["current", m.import_planner_mode_current(), m.import_planner_mode_current_hint()],
    ["each", m.import_planner_mode_each(), m.import_planner_mode_each_hint()],
    ["levelling", m.import_planner_mode_levelling(), m.import_planner_mode_levelling_hint()],
  ]);

  async function saveGameBuild() {
    commitAuthor();
    const state = await engine.getLoadouts().catch(() => null);
    plannerLoadouts = state?.loadouts.length ?? 0;
    plannerMode = plannerLoadouts > 1 ? (plannerLink?.mode ?? "current") : "current";
    plannerAuto = !!build.info?.file;
    plannerOpen = true;
  }

  async function runPlannerExport() {
    plannerBusy = true;
    try {
      const dir = gameBuilds?.dir ?? "";
      let target: string | null;
      if (plannerMode === "each") {
        const picked = await open({ directory: true, defaultPath: dir || undefined, title: m.import_planner_folder_title() });
        target = typeof picked === "string" ? picked : null;
      } else {
        const name = (build.info?.name ?? "build").replace(/[\\/:*?"<>|]/g, "");
        target = await save({ defaultPath: `${dir}/${name}.build`, filters: [{ name: "Game Build Planner", extensions: ["build"] }] });
        if (target && !/\.build$/i.test(target)) target += ".build";
      }
      if (!target) return;
      const { paths, files } = await planner.write(plannerMode, target);
      const file = build.info?.file;
      if (file) planner.setLink(file, plannerAuto ? { mode: plannerMode, target } : null);
      const f = files[0];
      say(files.length > 1 ? m.import_saved_build_files({ count: files.length }) : m.import_saved_build_file({ passives: f.passives, skills: f.skills, gear: f.gear }));
      build.say(m.import_saved_path({ path: paths.length > 1 ? target : paths[0] }));
      plannerOpen = false;
      refresh();
    } catch (e) {
      build.error = m.import_planner_export_failed({ error: String(e) });
    } finally {
      plannerBusy = false;
    }
  }

  function say(msg: string) {
    flash = msg;
    setTimeout(() => (flash = null), 2500);
  }

  function openBuild(b: BuildEntry) {
    noteRecent(b.path);
    return build.loadFile(b.path);
  }

  // A Mobalytics build page: the author's PoB build, if any, plus one Build
  // Planner file per variant, offered in a dialog rather than loaded blind.
  let moba = $state<MobalyticsBuild | null>(null);
  let mobaBusy = $state(false);

  async function loadCodeOrLink(codeOrLink: string) {
    if (/^https?:\/\//i.test(codeOrLink)) {
      const r = await fetchBuildCode(codeOrLink);
      say(m.import_fetched_from({ site: r.site }));
      return build.loadCode(r.code);
    }
    return build.loadCode(codeOrLink);
  }

  async function mobaImportPob() {
    if (!moba?.pobCode) return;
    mobaBusy = true;
    try {
      const ok = await loadCodeOrLink(moba.pobCode);
      if (ok) {
        moba = null;
        build.view = "tree";
      }
    } catch (e) {
      build.error = String(e);
    } finally {
      mobaBusy = false;
    }
  }

  async function mobaOpenVariant(v: MobalyticsVariant) {
    mobaBusy = true;
    try {
      await importGameBuildJson(v.json, v.name);
      if (!build.error) moba = null;
    } finally {
      mobaBusy = false;
    }
  }

  async function mobaSaveAll() {
    if (!moba) return;
    mobaBusy = true;
    try {
      const saved = await saveGameBuildFiles(moba.variants.map((v) => ({ name: v.name, json: v.json })), plannerDir || undefined);
      const replaced = saved.filter((s) => s.replaced).length;
      say(m.import_saved_planner_files({ count: saved.length, extra: replaced ? m.import_replaced_suffix({ count: replaced }) : "", folder: gameBuilds?.dir ?? m.import_game_folder_fallback() }));
      moba = null;
      await refresh();
    } catch (e) {
      build.error = String(e);
      say(m.import_save_failed({ error: String(e) }));
    } finally {
      mobaBusy = false;
    }
  }

  type CharSource = "ggg" | "ninja";
  interface CharRow {
    key: string;
    name: string;
    className: string;
    level: number;
    league: string;
    when: number;
    status: string;
    minLevel: number | null;
    leagueUrl: string;
    updated: string | null;
    ggg?: GameCharacter;
  }
  const CHAR_KEY = "pob-redux:character-import";
  let charAccount = $state("");
  let charRealm = $state("pc");
  let charSource = $state<CharSource>("ggg");
  let charList = $state<{ source: CharSource; game: string; account: string; rows: CharRow[] } | null>(null);
  let charLeague = $state("");
  let charBusy = $state<string | null>(null);
  try {
    const saved = JSON.parse(localStorage.getItem(CHAR_KEY) ?? "{}");
    charAccount = saved.account ?? "";
    charRealm = saved.realm ?? "pc";
    charSource = saved.source === "ninja" ? "ninja" : "ggg";
  } catch {}

  // PoE2 has no public character endpoint on pathofexile.com.
  const source = $derived<CharSource>(game.isPoe2 ? "ninja" : charSource);
  const shownList = $derived(charList && charList.source === source && charList.game === game.current ? charList : null);
  const charLeagues = $derived.by(() => {
    const seen = new Map<string, number>();
    for (const c of shownList?.rows ?? []) seen.set(c.league, Math.max(seen.get(c.league) ?? 0, c.when));
    return [...seen.entries()].sort((a, b) => b[1] - a[1]).map(([l]) => l);
  });
  const shownCharacters = $derived(
    (shownList?.rows ?? []).filter((c) => !charLeague || c.league === charLeague).sort((a, b) => b.when - a.when),
  );

  function ninjaReason(c: CharRow): string {
    switch (c.status) {
      case "belowCutoff":
        return m.import_reason_below_cutoff({ level: c.minLevel ?? 80 });
      case "leagueEnded":
        return m.import_reason_league_ended();
      case "inactive":
        return m.import_reason_inactive();
      case "notFetched":
        return m.import_reason_not_fetched();
      default:
        return m.import_reason_no_build();
    }
  }

  function shortDate(iso: string): string {
    const d = new Date(iso);
    return Number.isNaN(d.getTime()) ? "" : d.toLocaleDateString(undefined, { day: "numeric", month: "short" });
  }

  async function findCharacters() {
    if (!charAccount.trim() || charBusy) return;
    charBusy = "list";
    const src = source;
    const g = game.current;
    try {
      if (src === "ninja") {
        const r = await ninjaCharacters(charAccount);
        charList = {
          source: src,
          game: g,
          account: r.account,
          rows: r.characters.map((c) => ({
            key: `${c.league}/${c.name}`,
            name: c.name,
            className: c.className ?? "",
            level: c.level,
            league: c.league,
            when: (c.updated ? Date.parse(c.updated) : 0) || 0,
            status: c.status,
            minLevel: c.minLevel,
            leagueUrl: c.leagueUrl,
            updated: c.updated,
          })),
        };
      } else {
        const r = await characterList(charRealm, charAccount);
        charList = {
          source: src,
          game: g,
          account: r.account,
          rows: r.characters.map((c) => ({
            key: `${c.league}/${c.name}`,
            name: c.name,
            className: c.class,
            level: c.level,
            league: c.league,
            when: c.lastLoginTime ?? 0,
            status: "listed",
            minLevel: null,
            leagueUrl: "",
            updated: null,
            ggg: c,
          })),
        };
      }
      charAccount = charList.account;
      charLeague = charLeagues[0] ?? "";
      try {
        localStorage.setItem(CHAR_KEY, JSON.stringify({ account: charList.account, realm: charRealm, source: charSource }));
      } catch {}
    } catch (e) {
      charList = null;
      build.error = String(e);
    } finally {
      charBusy = null;
    }
  }

  async function importCharacter(c: CharRow) {
    const list = shownList;
    if (!list || charBusy) return;
    charBusy = c.key;
    try {
      const name = `${list.account.replace(/[#-]\d+$/, "")} - ${c.name}`;
      if (list.source === "ninja") {
        const code = await ninjaCharacterCode(list.account, c.name, c.leagueUrl);
        const r = await build.loadCode(code, name);
        if (r) build.say(c.updated ? m.import_imported_ninja({ name: c.name, date: shortDate(c.updated) }) : m.import_imported_ninja_plain({ name: c.name }));
      } else if (c.ggg) {
        const ggg = c.ggg;
        const d = await characterData(charRealm, list.account, c.name);
        const r = await build.run(() => engine.importCharacter({ character: ggg, passives: d.passives, items: d.items, name }));
        if (r) {
          build.say(m.import_imported({ name: c.name }));
          build.view = "tree";
        }
      }
    } catch (e) {
      build.error = String(e);
    } finally {
      charBusy = null;
    }
  }

  let updating = $state<string | null>(null);
  let reimport = $state<ReimportOptions>({ tree: true, gear: true, deleteJewels: true, deleteEquipment: true, deleteSkills: true, ignoreWeaponSwap: false });

  async function reimportCharacter(c: CharRow) {
    const list = shownList;
    if (!list || charBusy || !build.info) return;
    charBusy = c.key;
    try {
      let r;
      if (list.source === "ninja") {
        const code = await ninjaCharacterCode(list.account, c.name, c.leagueUrl);
        r = await build.run(() => engine.mergeBuild({ code, ...reimport }));
      } else if (c.ggg) {
        const ggg = c.ggg;
        const d = await characterData(charRealm, list.account, c.name);
        r = await build.run(() => engine.importCharacterInto({ character: ggg, passives: d.passives, items: d.items, ...reimport }));
      }
      if (r) {
        updating = null;
        build.say(m.import_reimported({ name: c.name }));
      }
    } catch (e) {
      build.error = String(e);
    } finally {
      charBusy = null;
    }
  }

  // A Maxroll guide with more than one PoB link: the user picks which to load.
  let guide = $state<MaxrollGuide | null>(null);
  let guideBusy = $state(false);

  async function guideImport(l: MaxrollPobLink) {
    guideBusy = true;
    try {
      if (await loadCodeOrLink(l.url)) guide = null;
    } catch (e) {
      build.error = String(e);
    } finally {
      guideBusy = false;
    }
  }

  async function doImport() {
    const text = code.trim();
    if (!text) return;
    if (isMobalyticsLink(text)) {
      fetching = true;
      try {
        moba = await resolveMobalytics(text);
      } catch (e) {
        build.error = String(e);
      } finally {
        fetching = false;
      }
    } else if (isMaxrollGuideLink(text)) {
      fetching = true;
      try {
        const r = await resolveMaxroll(text);
        if (r.links.length === 1) await loadCodeOrLink(r.links[0].url);
        else guide = r;
      } catch (e) {
        build.error = String(e);
      } finally {
        fetching = false;
      }
    } else if (/^https?:\/\//i.test(text)) {
      fetching = true;
      try {
        await loadCodeOrLink(text);
      } catch (e) {
        build.error = String(e);
      } finally {
        fetching = false;
      }
    } else if (text.startsWith("<")) {
      await build.loadXml(text);
    } else {
      await build.loadCode(text);
    }
  }

  async function pasteImport() {
    const t = (await readText().catch(() => "")) ?? "";
    if (t) {
      code = t;
      await doImport();
    }
  }

  async function copyToClipboard(text: string, what: string) {
    try {
      await writeText(text);
      say(m.import_copied({ what }));
    } catch (e) {
      // The clipboard can be held by another app; the code box is the fallback.
      code = text;
      say(m.import_clipboard_failed({ error: String(e), what: what.toLowerCase() }));
    }
  }

  async function copyCode() {
    const r = await build.run(() => engine.saveBuildCode(), { sync: false });
    if (r) await copyToClipboard(r.code, m.import_what_code());
  }

  async function shareLink() {
    const r = await build.run(() => engine.saveBuildCode(), { sync: false });
    if (!r) return;
    sharing = true;
    shareUrl = null;
    try {
      localStorage.setItem(SITE_KEY, shareSite);
    } catch {}
    try {
      const link = await shareBuildCode(shareSite, r.code);
      shareUrl = link.url;
      await copyToClipboard(link.url, `${link.site} link`);
    } catch (e) {
      build.error = m.import_share_link_failed({ error: String(e) });
    } finally {
      sharing = false;
    }
  }

  async function openXml() {
    let p: string | string[] | null;
    try {
      p = await open({
        multiple: false,
        filters: [{ name: "Builds", extensions: ["xml", "build"] }],
      });
    } catch (e) {
      build.error = m.import_open_dialog_failed({ error: String(e) });
      return;
    }
    if (typeof p === "string") {
      if (p.toLowerCase().endsWith(".build")) {
        await importGameBuildFile(p, p.replace(/^.*[\\/]/, "").replace(/\.build$/i, ""));
      } else {
        noteRecent(p);
        await build.loadFile(p);
      }
    }
  }

  async function saveAs() {
    const r = await build.saveAs();
    if (r) {
      noteRecent(r.path);
      say(m.import_saved_path({ path: r.path }));
      refresh();
    }
  }

  async function saveCurrent() {
    const r = await build.save();
    if (r) {
      noteRecent(r.path);
      say(m.import_saved_path({ path: r.path }));
      refresh();
    }
  }

  async function exportXml() {
    const r = await build.run(() => engine.saveBuildXml(), { sync: false, user: false });
    if (!r) return;
    try {
      const p = await save({ defaultPath: `${paths?.builds_dir ?? ""}/${build.info?.name ?? "build"}.xml`, filters: [{ name: "XML", extensions: ["xml"] }] });
      if (!p) return;
      await writeTextFile(p, r.xml);
      say(m.import_exported_path({ path: p }));
      build.say(m.import_exported_path({ path: p }));
    } catch (e) {
      build.error = m.import_export_failed({ error: String(e) });
    }
  }

  async function commitRename(b: BuildEntry) {
    const name = renameDraft.trim();
    renaming = null;
    if (!name || name === b.name) return;
    try {
      const np = await renameBuild(b.path, name);
      say(m.import_renamed_to({ name }));
      if (build.info?.file === b.path) await build.loadFile(np);
      refresh();
    } catch (e) {
      build.error = String(e);
    }
  }

  async function commitMove(b: BuildEntry, folder: string) {
    moving = null;
    movingNew = null;
    if (folder === b.folder) return;
    try {
      const np = await moveBuild(b.path, folder);
      say(m.import_moved_to({ folder: folder || m.import_top_level_name() }));
      if (build.info?.file === b.path) await build.loadFile(np);
      refresh();
    } catch (e) {
      build.error = String(e);
    }
  }

  async function commitDelete(b: BuildEntry) {
    confirmDelete = null;
    try {
      await deleteBuild(b.path);
      recent = recent.filter((p) => p !== b.path);
      say(m.import_deleted({ name: b.name }));
      refresh();
    } catch (e) {
      build.error = String(e);
    }
  }

  async function restoreAutosave() {
    if (!autosave) return;
    await build.loadXml(autosave.xml, autosave.name);
    say(m.import_autosave_restored());
  }

  function fmtDate(t: number) {
    const d = new Date(t * 1000);
    return d.toLocaleDateString(undefined, { year: "2-digit", month: "short", day: "numeric" });
  }
  function fmtTime(ms: number) {
    return new Date(ms).toLocaleString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  }
</script>

<svelte:window onkeydown={onFindKey} />

{#snippet buildRow(b: BuildEntry, showFolder: boolean)}
  <div class="row">
    {#if renaming === b.path}
      <input
        class="input grow"
        bind:value={renameDraft}
        onblur={() => commitRename(b)}
        onkeydown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          if (e.key === "Escape") (renaming = null);
        }}
      />
    {:else}
      <button class="name" onclick={() => openBuild(b)} disabled={build.busy > 0}>
        {#if artVersion && b.class_name}
          <ClassIcon
            game={game.current}
            version={artVersion}
            className={ascendByKey.get(b.ascend_class_name ?? "")?.cls ?? b.class_name}
            ascendancy={ascendByKey.get(b.ascend_class_name ?? "")?.asc ?? b.ascend_class_name}
            size={20}
          />
        {:else}
          <span class="noart"></span>
        {/if}
        <span class="ntext">{#if showFolder && b.folder}<span class="dim">{b.folder}/</span>{/if}{b.name}</span>
      </button>
      <span class="meta">
        <span class="cls" style:color={buildColour(b.class_name, b.ascend_class_name)} title={[b.class_name, b.ascend_class_name].filter(Boolean).join(" · ")}>{ascendByKey.get(b.ascend_class_name ?? "")?.asc ?? b.ascend_class_name ?? b.class_name ?? "?"}</span>
        <span class="lvl num dim">L{b.level ?? "?"}</span>
        <span class="date num dim">{fmtDate(b.modified)}</span>
      </span>
      <span class="acts" class:open={moving === b.path || movingNew === b.path || confirmDelete === b.path}>
        {#if moving === b.path}
          <select
            class="select xs"
            value={b.folder}
            onchange={(e) => {
              const v = (e.target as HTMLSelectElement).value;
              if (v === "__new") {
                moving = null;
                movingNew = b.path;
                folderDraft = "";
              } else commitMove(b, v);
            }}
            onblur={() => (moving = null)}
          >
            <option value="">{m.import_top_level()}</option>
            {#each folders as f}
              <option value={f}>{f}</option>
            {/each}
            <option value="__new">{m.import_new_folder_option()}</option>
          </select>
        {:else if movingNew === b.path}
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="input xs fdraft"
            placeholder={m.import_folder_name()}
            bind:value={folderDraft}
            autofocus
            onkeydown={(e) => {
              if (e.key === "Enter" && folderDraft.trim()) commitMove(b, folderDraft.trim());
              if (e.key === "Escape") (movingNew = null);
            }}
          />
          <button class="act" disabled={!folderDraft.trim()} onclick={() => commitMove(b, folderDraft.trim())}>{m.import_move()}</button>
          <button class="act" onclick={() => (movingNew = null)}>{m.import_cancel()}</button>
        {:else if confirmDelete === b.path}
          <button class="act danger" onclick={() => commitDelete(b)}>{m.import_confirm()}</button>
          <button class="act" onclick={() => (confirmDelete = null)}>{m.import_keep()}</button>
        {:else}
          <button class="ibtn" title={m.common_rename()} aria-label={m.common_rename()} onclick={() => { renaming = b.path; renameDraft = b.name; }}><Icon name="pencil" size={13} /></button>
          <button class="ibtn" title={m.import_move_title()} aria-label={m.import_move_title()} onclick={() => (moving = b.path)}><Icon name="folder" size={13} /></button>
          <button class="ibtn" title={m.common_delete()} aria-label={m.common_delete()} onclick={() => (confirmDelete = b.path)}><Icon name="trash" size={13} /></button>
        {/if}
      </span>
    {/if}
  </div>
{/snippet}

<div class="page">
  <section class="col">
    <div class="toolbar">
      <span class="hinted">
        <input class="input" placeholder={m.import_filter()} bind:value={filter} bind:this={filterEl} />
        {#if !filter}<Kbd keys="Mod+F" hint />{/if}
      </span>
      <select class="select" bind:value={sort} title={m.import_sort()}>
        <option value="modified">{m.import_sort_recent()}</option>
        <option value="name">{m.import_sort_name()}</option>
        <option value="level">{m.import_sort_level()}</option>
        <option value="class">{m.import_sort_class()}</option>
      </select>
      <span class="vr"></span>
      <button class="btn sm" onclick={openXml}>{m.import_open_file()}</button>
      <button class="btn sm" onclick={() => (newFolder = "")}>{m.import_new_folder()}</button>
      <button class="btn sm primary" onclick={() => build.newBuild().then((r) => r && (ui.newBuildOpen = true))} disabled={build.busy > 0}>{m.import_new_build()}</button>
    </div>
    <div class="list">
      {#if newFolder !== null}
        <div class="newdir">
          <span class="label">{m.import_new_folder()}</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input
            class="input grow"
            placeholder={m.import_folder_placeholder()}
            value={newFolder}
            autofocus
            oninput={(e) => (newFolder = (e.target as HTMLInputElement).value)}
            onkeydown={(e) => {
              if (e.key === "Enter") commitNewFolder();
              if (e.key === "Escape") (newFolder = null);
            }}
          />
          <button class="btn sm primary" disabled={!newFolder.trim()} onclick={commitNewFolder}>{m.common_create()}</button>
          <button class="btn sm ghost" onclick={() => (newFolder = null)}>{m.common_cancel()}</button>
          <span class="hint">{m.import_folder_hint()}</span>
        </div>
      {/if}
      {#if autosave && autosave.name !== build.info?.name}
        <div class="recover">
          <span>{m.import_autosave()} <b>{autosave.name}</b> · {fmtTime(autosave.at)}</span>
          <span class="acts2">
            <button class="btn sm" onclick={restoreAutosave} disabled={build.busy > 0}>{m.import_restore()}</button>
            <button class="btn sm ghost" onclick={() => { localStorage.removeItem(autosaveKey()); autosave = null; }}>{m.import_dismiss()}</button>
          </span>
        </div>
      {/if}
      {#if recentEntries.length && !filter.trim()}
        <div class="srchead" title={m.import_recent_desc()}>
          <div class="srctitle">{m.import_recent_title()}</div>
        </div>
        {#each recentEntries as b (b.path)}
          {@render buildRow(b, true)}
        {/each}
      {/if}

      <div class="srchead">
        <div class="srctitle" title={m.import_pob_desc()}>
          {m.import_pob_title()}
          <span class="count num">{builds.length}</span>
        </div>
        {#if paths?.builds_dir}
          <div class="srcpath">
            <span class="picon"><Icon name="folder" size={13} /></span>
            <code class="mono ptext" title={paths.builds_dir}>{paths.builds_dir}</code>
            <button class="btn sm" title={m.import_builds_dir_title()} onclick={browseBuildsDir}>{m.import_browse()}</button>
            <button class="btn sm" onclick={() => paths && reveal(paths.builds_dir)}>{m.import_show_folder()}</button>
            {#if paths.builds_dir !== paths.default_builds_dir}
              <button class="btn sm" title={m.import_builds_reset_title({ path: paths.default_builds_dir })} onclick={() => changeBuildsDir(null)}>{m.import_reset()}</button>
            {/if}
          </div>
        {/if}
      </div>
      {#if shown.length === 0}
        <div class="dim small pad">{m.import_no_builds()}</div>
      {/if}
      {#each grouped as [folder, items] (folder)}
        {#if !rootOnly}
          <div class="ghead ghrow">
            {#if renamingFolder === folder}
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="input xs fdraft"
                aria-label={m.import_folder_rename_title()}
                bind:value={folderRenameDraft}
                autofocus
                onblur={() => commitRenameFolder(folder)}
                onkeydown={(e) => {
                  if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                  if (e.key === "Escape") (renamingFolder = null);
                }}
              />
            {:else}
              <button class="ghtoggle" aria-expanded={folderOpen(folder)} onclick={() => toggleFolder(folder)}>
                <span class="caret" class:open={folderOpen(folder)}>▸</span>
                {#if folder !== ""}<Icon name="folder" size={12} />{/if}
                <span>{folder === "" ? m.import_top_level_group() : folder}</span>
                <span class="dim num">{items.length}</span>
              </button>
            {/if}
            {#if folder !== "" && renamingFolder !== folder}
              {#if confirmFolder === folder}
                <button class="act danger" onclick={() => commitDeleteFolder(folder)}>{m.import_confirm()}</button>
                <button class="act" onclick={() => (confirmFolder = null)}>{m.import_keep()}</button>
              {:else}
                <button
                  class="ibtn"
                  title={m.import_folder_new_build_title()}
                  aria-label={m.import_folder_new_build_title()}
                  disabled={build.busy > 0}
                  onclick={() => build.newBuild(undefined, folder).then((r) => r && (ui.newBuildOpen = true))}><Icon name="plus-square" size={13} /></button
                >
                <button
                  class="ibtn"
                  title={m.import_folder_rename_title()}
                  aria-label={m.import_folder_rename_title()}
                  onclick={() => {
                    renamingFolder = folder;
                    folderRenameDraft = folder.split("/").pop() ?? folder;
                  }}><Icon name="pencil" size={13} /></button
                >
                <button
                  class="ibtn"
                  title={folderCount(folder) ? m.import_folder_not_empty() : m.import_folder_delete_title()}
                  aria-label={m.import_folder_delete_title()}
                  disabled={folderCount(folder) > 0}
                  onclick={() => (confirmFolder = folder)}><Icon name="trash" size={13} /></button
                >
              {/if}
            {/if}
          </div>
        {/if}
        {#if rootOnly || folderOpen(folder)}
          {#each items as b (b.path)}
            {@render buildRow(b, false)}
          {/each}
          {#if items.length === 0}
            <div class="dim small pad">{m.import_empty_folder()}</div>
          {/if}
        {/if}
      {/each}

      {#if game.isPoe2}
        <div class="srchead">
          <div class="srctitle" title={m.import_planner_desc()}>
            {m.import_planner()}
            <span class="count num">{gameBuilds?.builds.length ?? 0}</span>
          </div>
          <div class="srcpath">
            <span class="picon"><Icon name="folder" size={13} /></span>
            <input
              class="input mono pathin"
              value={plannerDir || gameBuilds?.dir || ""}
              placeholder={m.import_planner_dir_placeholder()}
              aria-label={m.import_planner()}
              onchange={(e) => setPlannerDir((e.target as HTMLInputElement).value)}
              onkeydown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
            />
            <button class="btn sm" onclick={browsePlannerDir}>{m.import_browse()}</button>
            {#if gameBuilds?.exists}
              <button class="btn sm" onclick={() => gameBuilds && reveal(gameBuilds.dir)}>{m.import_show_folder()}</button>
            {/if}
            {#if plannerDir}
              <button class="btn sm" title={m.import_planner_reset_title()} onclick={() => setPlannerDir("")}>{m.import_reset()}</button>
            {/if}
          </div>
          {#if gameBuilds && !gameBuilds.exists}
            <p class="srcwarn">{m.import_planner_missing()}</p>
          {/if}
        </div>
        {#each gameBuildGroups as [group, items] (group)}
          {#if editAuthor === group}
            <div class="ahead editing">
              <span class="caret open">▸</span>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="input grow"
                bind:value={authorDraft}
                placeholder={m.import_author_placeholder()}
                autofocus
                onblur={() => commitGroupAuthor(items)}
                onkeydown={(e) => {
                  if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                  if (e.key === "Escape") (editAuthor = null);
                }}
              />
            </div>
          {:else}
            <div class="ahead">
              <button class="ahead-toggle" onclick={() => toggleAuthor(group)}>
                <span class="caret" class:open={!gbCollapsed.has(group)}>▸</span>
                <span class="aname">{group}</span>
                <span class="dim num">{items.length}</span>
              </button>
              <button
                class="ibtn"
                title={m.import_author_group_title()}
                aria-label={m.import_author_group_title()}
                onclick={() => {
                  authorDraft = items[0]?.author ?? "";
                  editAuthor = group;
                }}><Icon name="user" size={13} /></button
              >
            </div>
          {/if}
          {#if !gbCollapsed.has(group)}
            {#each items as gb (gb.path)}
              <div class="row gbrow">
                {#if gbEdit?.path === gb.path}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="input grow gbedit"
                    bind:value={gbDraft}
                    placeholder={gbEdit.field === "name" ? m.import_build_name() : m.import_author_placeholder()}
                    autofocus
                    onblur={() => commitGbEdit(gb)}
                    onkeydown={(e) => {
                      if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                      if (e.key === "Escape") (gbEdit = null);
                    }}
                  />
                {:else}
                  {@const who = gb.ascendancy ? ascendByKey.get(gb.ascendancy) : undefined}
                  <button class="name" onclick={() => importGameBuildFile(gb.path, gb.name)} disabled={build.busy > 0}>
                    {#if artVersion && who}
                      <ClassIcon game={game.current} version={artVersion} className={who.cls} ascendancy={who.asc} size={20} />
                    {:else}
                      <span class="noart"></span>
                    {/if}
                    <span class="ntext">{gb.name}</span>
                  </button>
                  <span class="meta">
                    <span class="cls" style:color={who ? buildColour(who.cls, who.asc) : undefined} title={who ? `${who.cls} · ${who.asc}` : undefined}>{who?.asc ?? ""}</span>
                    <span class="lvl"></span>
                    <span class="date num dim">{fmtDate(gb.modified)}</span>
                  </span>
                  <span class="acts">
                    <button class="ibtn" title={m.import_gb_rename_title()} aria-label={m.common_rename()} onclick={() => { gbDraft = gb.name; gbEdit = { path: gb.path, field: "name" }; }}><Icon name="pencil" size={13} /></button>
                    <button class="ibtn" title={m.import_gb_author_title()} aria-label={m.import_gb_author_title()} onclick={() => { gbDraft = gb.author ?? ""; gbEdit = { path: gb.path, field: "author" }; }}><Icon name="user" size={13} /></button>
                  </span>
                {/if}
              </div>
            {/each}
          {/if}
        {/each}
      {/if}
    </div>
  </section>

  <section class="col side">
    <div class="panel-head"><span class="label">{m.import_title()}</span></div>
    <div class="block">
      <textarea
        class="textarea"
        rows="5"
        placeholder={m.import_code_placeholder()}
        bind:value={code}
      ></textarea>
      <div class="actions">
        <button class="btn primary" onclick={doImport} disabled={!code.trim() || build.busy > 0 || fetching}>
          {fetching ? m.import_fetching() : m.import_title()}
        </button>
        <button class="btn" onclick={pasteImport} disabled={build.busy > 0 || fetching}>{m.import_paste_and_import()}</button>
      </div>
    </div>

    <div class="panel-head">
      <span class="label">{m.import_character()}</span>
      {#if !game.isPoe2}
        <select class="select sm" bind:value={charSource} title={m.import_source_title()}>
          <option value="ggg">pathofexile.com</option>
          <option value="ninja">poe.ninja</option>
        </select>
      {/if}
    </div>
    <div class="block">
      <div class="charform">
        <input
          class="input grow"
          placeholder={m.import_account_placeholder()}
          bind:value={charAccount}
          onkeydown={(e) => e.key === "Enter" && findCharacters()}
        />
        {#if source === "ggg"}
          <select class="select" bind:value={charRealm} title={m.import_realm()}>
            <option value="pc">{m.import_realm_pc()}</option>
            <option value="xbox">{m.import_realm_xbox()}</option>
            <option value="sony">{m.import_realm_sony()}</option>
          </select>
        {/if}
        <button class="btn" onclick={findCharacters} disabled={!charAccount.trim() || charBusy !== null}>
          {charBusy === "list" ? m.import_finding() : m.import_find()}
        </button>
      </div>
      {#if shownList}
        <div class="charhead">
          <select class="select sm" bind:value={charLeague}>
            {#each charLeagues as l}<option value={l}>{l}</option>{/each}
            <option value="">{m.import_all_leagues()}</option>
          </select>
          <span class="dim small"><span class="num">{shownCharacters.length}</span> {m.import_characters()}</span>
        </div>
        <div class="charlist">
          {#each shownCharacters as c (c.key)}
            {@const who = ascendByKey.get(c.className) ?? (build.classes.some((k) => k.name === c.className) ? { cls: c.className, asc: null } : undefined)}
            <div class="charrow">
              {#if artVersion && who}
                <ClassIcon game={game.current} version={artVersion} className={who.cls} ascendancy={who.asc} size={18} />
              {:else}
                <span class="noart small-art"></span>
              {/if}
              <span class="cname" title={c.updated ? m.import_char_title_saved({ name: c.name, league: c.league, date: shortDate(c.updated) }) : m.import_char_title({ name: c.name, league: c.league })}>
                {c.name}{#if !charLeague}<span class="dim small cleague">{c.league}</span>{/if}
              </span>
              <span class="dim small"><span style:color={classColour.get(c.className)}>{c.className}</span> <span class="num">{c.level}</span></span>
              {#if c.status === "listed"}
                {#if build.info}
                  <button
                    class="act"
                    class:on={updating === c.key}
                    aria-expanded={updating === c.key}
                    title={m.import_update_open_title()}
                    onclick={() => (updating = updating === c.key ? null : c.key)}
                    disabled={charBusy !== null || build.busy > 0}>{m.import_update_open()}</button
                  >
                {/if}
                <button class="act" onclick={() => importCharacter(c)} disabled={charBusy !== null || build.busy > 0}>
                  {charBusy === c.key && updating !== c.key ? m.import_importing() : m.import_import_short()}
                </button>
              {:else}
                <span class="dim small nobuild" title={m.import_no_ninja_build()}>{ninjaReason(c)}</span>
              {/if}
            </div>
            {#if updating === c.key && build.info}
              <div class="reimport">
                <div class="rpart">
                  <label class="chk small"><input type="checkbox" bind:checked={reimport.tree} /> {m.import_reimport_tree()}</label>
                  <label class="chk small sub"><input type="checkbox" bind:checked={reimport.deleteJewels} disabled={!reimport.tree} /> {m.import_reimport_delete_jewels()}</label>
                </div>
                <div class="rpart">
                  <label class="chk small"><input type="checkbox" bind:checked={reimport.gear} /> {m.import_reimport_gear()}</label>
                  <label class="chk small sub"><input type="checkbox" bind:checked={reimport.deleteEquipment} disabled={!reimport.gear} /> {m.import_reimport_delete_equipment()}</label>
                  <label class="chk small sub"><input type="checkbox" bind:checked={reimport.deleteSkills} disabled={!reimport.gear} /> {m.import_reimport_delete_skills()}</label>
                  <label class="chk small sub"><input type="checkbox" bind:checked={reimport.ignoreWeaponSwap} disabled={!reimport.gear} /> {m.import_reimport_ignore_swap()}</label>
                </div>
                <div class="rgo">
                  <span class="dim small">{m.import_reimport_into({ name: build.info.name })}</span>
                  <button
                    class="btn sm primary"
                    onclick={() => reimportCharacter(c)}
                    disabled={(!reimport.tree && !reimport.gear) || charBusy !== null || build.busy > 0}
                  >{charBusy === c.key ? m.import_updating() : m.import_update()}</button>
                </div>
              </div>
            {/if}
          {/each}
        </div>
      {:else if source === "ninja"}
        <div class="dim small">{m.import_ninja_blurb()}</div>
      {:else}
        <div class="dim small">{m.import_ggg_blurb()}</div>
      {/if}
    </div>

    {#if plannerOpen}
      <div class="overlay" role="presentation" onclick={() => !plannerBusy && (plannerOpen = false)} onkeydown={(e) => e.key === "Escape" && !plannerBusy && (plannerOpen = false)}>
        <div class="modal" role="dialog" aria-label={m.import_planner_dialog()} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && !plannerBusy && (plannerOpen = false)}>
          <div class="mhead">
            <span class="label">{m.import_planner_dialog()}</span>
            <button class="btn sm ghost" onclick={() => (plannerOpen = false)} disabled={plannerBusy}>{m.common_close()}</button>
          </div>
          <div class="mbody">
            {#if plannerLoadouts > 1}
              {#each plannerModes as [id, title, hint] (id)}
                <label class="mrow pmode">
                  <input type="radio" name="planner-mode" value={id} bind:group={plannerMode} disabled={plannerBusy} />
                  <span class="mtext"><span class="mtitle">{title}</span><span class="dim small">{hint}</span></span>
                </label>
              {/each}
            {/if}
            <label class="mrow pmode">
              <input type="checkbox" bind:checked={plannerAuto} disabled={plannerBusy || !build.info?.file} />
              <span class="mtext">
                <span class="mtitle">{m.import_planner_auto()}</span>
                <span class="dim small">{build.info?.file ? m.import_planner_auto_hint() : m.import_planner_auto_unsaved()}</span>
              </span>
            </label>
            <div class="mrow">
              <span class="mtext dim small">{plannerLink ? m.import_planner_linked({ target: plannerLink.target }) : ""}</span>
              {#if plannerLink && build.info?.file}
                <button class="btn sm ghost" onclick={() => build.info?.file && planner.setLink(build.info.file, null)} disabled={plannerBusy}>{m.import_planner_stop()}</button>
              {/if}
              <button class="btn primary sm" onclick={runPlannerExport} disabled={plannerBusy}>{m.import_planner_save()}</button>
            </div>
          </div>
        </div>
      </div>
    {/if}

    {#if moba}
      <div class="overlay" role="presentation" onclick={() => !mobaBusy && (moba = null)} onkeydown={(e) => e.key === "Escape" && (moba = null)}>
        <div class="modal" role="dialog" aria-label={m.import_moba_dialog()} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (moba = null)}>
          <div class="mhead">
            <span class="label">{m.import_moba()}</span>
            <span class="dim small mono">{moba.slug}</span>
            <button class="btn sm ghost" onclick={() => (moba = null)} disabled={mobaBusy}>{m.common_close()}</button>
          </div>
          <div class="mbody">
            {#if moba.pobCode}
              <div class="mrow">
                <div class="mtext">
                  <div class="mtitle">{m.import_moba_pob()}</div>
                  <div class="dim small">{m.import_moba_pob_blurb()}</div>
                </div>
                <button class="btn primary sm" onclick={mobaImportPob} disabled={mobaBusy || build.busy > 0}>{m.import_title()}</button>
              </div>
            {/if}
            {#if moba.variants.length}
              <div class="mrow">
                <div class="mtext">
                  <div class="mtitle">{m.import_moba_planner()} <span class="dim num">{moba.variants.length}</span></div>
                  <div class="dim small">
                    {m.import_moba_planner_blurb({ folder: gameBuilds?.dir ?? m.import_moba_folder_fallback() })}
                    {#if !moba.pobCode}{m.import_moba_planner_warning()}{/if}
                  </div>
                </div>
                <button class="btn sm" class:primary={!moba.pobCode} onclick={mobaSaveAll} disabled={mobaBusy}>{m.import_moba_save_all()}</button>
              </div>
              <div class="mvariants">
                {#each moba.variants as v (v.id)}
                  <div class="mvar">
                    <span class="mvname" title={v.name}>{v.name}</span>
                    <span class="dim small num">{m.import_moba_variant({ passives: v.passives, skills: v.skills })}</span>
                    <button class="act" onclick={() => mobaOpenVariant(v)} disabled={mobaBusy || build.busy > 0}>{m.import_moba_open_here()}</button>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        </div>
      </div>
    {/if}

    {#if guide}
      <div class="overlay" role="presentation" onclick={() => !guideBusy && (guide = null)} onkeydown={(e) => e.key === "Escape" && (guide = null)}>
        <div class="modal" role="dialog" aria-label={m.import_maxroll_dialog()} tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.key === "Escape" && (guide = null)}>
          <div class="mhead">
            <span class="label">{m.import_maxroll()}</span>
            <span class="dim small mtrunc" title={guide.title}>{guide.title}</span>
            <button class="btn sm ghost" onclick={() => (guide = null)} disabled={guideBusy}>{m.common_close()}</button>
          </div>
          <div class="mbody">
            {#each guide.links as l, i (l.url)}
              <div class="mrow">
                <div class="mtext">
                  <div class="mtitle">{l.name}</div>
                  <div class="dim small mtrunc" title={l.url}>{l.source} · <span class="mono">{l.url.replace(/^https?:\/\//, "")}</span></div>
                </div>
                <button class="btn sm" class:primary={i === 0} onclick={() => guideImport(l)} disabled={guideBusy || build.busy > 0}>{m.import_title()}</button>
              </div>
            {/each}
          </div>
        </div>
      </div>
    {/if}

    {#if build.loaded}
      <div class="panel-head">
        <span class="label">{m.import_current()}</span>
        {#if build.info?.unsaved}<span class="dim small">{m.import_unsaved()}</span>{/if}
      </div>
      <div class="block current">
        <div class="fld">
          <span class="label">{m.import_name()}</span>
          {#if nameDraft !== null}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="input grow"
              bind:value={nameDraft}
              autofocus
              onblur={commitBuildName}
              onkeydown={(e) => {
                if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                if (e.key === "Escape") (nameDraft = null);
              }}
            />
          {:else}
            <button
              class="bname grow"
              onclick={() => (nameDraft = build.info?.name ?? "")}
              title={(build.info?.file ? `${build.info.file}\n` : m.import_not_saved()) + m.import_click_rename()}
            >{build.info?.name ?? "—"}</button>
          {/if}
        </div>
        <div class="fld">
          <span class="label">{m.import_author_label()}</span>
          <input class="input grow" bind:value={author} placeholder="—" title={m.import_author_title()} onblur={commitAuthor} />
        </div>
        <div class="arow">
          <span class="label">{m.import_save()}</span>
          <button class="btn sm" onclick={saveCurrent} title={build.info?.file ?? m.import_save_title()}>{m.import_save()}</button>
          <button class="btn sm" onclick={saveAs}>{m.import_save_as()}</button>
        </div>
        <div class="arow">
          <span class="label">{m.import_share()}</span>
          <button class="btn sm" onclick={copyCode}>{m.import_copy_code()}</button>
          <span class="joined">
            <button class="btn sm" onclick={shareLink} disabled={sharing} title={m.import_link_title()}>{sharing ? m.import_creating_link() : m.import_link()}</button>
            <select class="select xs" bind:value={shareSite} disabled={sharing}>
              {#each SHARE_SITES as site}<option value={site}>{site}</option>{/each}
            </select>
          </span>
        </div>
        <div class="arow">
          <span class="label">{m.import_export()}</span>
          <button class="btn sm" onclick={exportXml}>{m.import_export_xml()}</button>
          {#if game.isPoe2}
            <button class="btn sm" onclick={saveGameBuild} title={m.import_export_planner_title()}>{m.import_export_planner()}</button>
          {/if}
        </div>
        {#if shareUrl}
          <div class="arow">
            <input class="input grow mono" readonly value={shareUrl} onfocus={(e) => (e.target as HTMLInputElement).select()} />
            <button class="btn sm" onclick={() => copyToClipboard(shareUrl!, m.import_what_link())}>{m.common_copy_button()}</button>
          </div>
        {/if}
      </div>
    {/if}
    {#if flash}<div class="flash">{flash}</div>{/if}
  </section>
</div>

<style>
  .page {
    flex: 1;
    display: grid;
    grid-template-columns: minmax(360px, 1fr) minmax(360px, 460px);
    min-height: 0;
  }
  .col {
    display: flex;
    flex-direction: column;
    min-height: 0;
    border-right: 1px solid var(--line-0);
  }
  .col:last-child {
    border-right: 0;
    background: var(--bg-1);
  }
  .toolbar {
    display: flex;
    gap: 6px;
    padding: 8px 10px;
    border-bottom: 1px solid var(--line-0);
  }
  .toolbar .input {
    flex: 1;
    min-width: 100px;
  }
  .toolbar .hinted {
    position: relative;
    display: flex;
    flex: 1;
    min-width: 100px;
  }
  .vr {
    width: 1px;
    align-self: stretch;
    margin: 2px 2px;
    background: var(--line-1);
  }
  .fdraft {
    width: 150px;
    flex: 0 0 auto;
  }
  .list {
    flex: 1;
    overflow-y: auto;
  }
  .ghead {
    padding: 7px 12px 3px;
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    color: var(--fg-3);
    position: sticky;
    top: 0;
    background: var(--bg-0);
    z-index: 1;
  }
  .srchead {
    padding: 9px 12px;
    border-top: 1px solid var(--line-2);
    border-bottom: 1px solid var(--line-1);
    background: var(--bg-3);
  }
  .list > .srchead:first-child {
    border-top: 0;
  }
  .srctitle {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-size: var(--fs-md);
    font-weight: 600;
    color: var(--fg-0);
  }
  .count {
    font-size: var(--fs-xs);
    font-weight: 400;
    color: var(--fg-2);
  }
  .srcpath {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 7px;
  }
  .picon {
    display: grid;
    color: var(--fg-2);
  }
  .ptext {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
  .pathin {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-xs);
  }
  .srcwarn {
    margin: 6px 0 0;
    font-size: var(--fs-xs);
    color: var(--warn);
  }
  .ibtn {
    appearance: none;
    display: grid;
    place-items: center;
    flex: none;
    width: 24px;
    height: 22px;
    padding: 0;
    border: 0;
    border-radius: var(--r-1);
    background: none;
    color: var(--fg-2);
    cursor: pointer;
  }
  .ibtn:hover:not(:disabled) {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .ibtn:disabled {
    opacity: var(--fade-off);
    cursor: default;
  }
  .aname {
    font-size: var(--fs-md);
    letter-spacing: 0;
    color: var(--c-author);
  }
  .ahead {
    appearance: none;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px 4px;
    border: 0;
    background: transparent;
    color: var(--fg-1);
    font-size: var(--fs-xs);
    font-weight: 600;
    letter-spacing: 0.03em;
    cursor: pointer;
    text-align: left;
  }
  .ahead .ahead-toggle {
    appearance: none;
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    border: 0;
    background: transparent;
    color: inherit;
    font: inherit;
    letter-spacing: inherit;
    cursor: pointer;
    text-align: left;
    padding: 0;
  }
  .ahead .ibtn {
    opacity: 0;
  }
  .ahead:hover .ibtn,
  .ahead .ibtn:focus-visible {
    opacity: 1;
  }
  .ahead.editing {
    padding-right: 10px;
  }
  .ahead:hover {
    color: var(--fg-0);
  }
  .joined {
    display: inline-flex;
    align-items: stretch;
  }
  .joined .btn {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .joined .select {
    height: auto;
    border-left: 0;
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
  }
  .caret {
    display: inline-block;
    width: 14px;
    font-size: 15px;
    line-height: 1;
    color: var(--fg-2);
    transition: transform 100ms;
  }
  .ghrow {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .ghrow .ibtn {
    opacity: 0;
  }
  .ghrow:hover .ibtn,
  .ghrow .ibtn:focus-visible {
    opacity: 1;
  }
  .ghtoggle {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
    padding: 0;
    background: none;
    border: 0;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: inherit;
    text-align: left;
    cursor: pointer;
  }
  .ghtoggle:hover {
    color: var(--fg-1);
  }
  .ghtoggle:hover .caret {
    color: var(--fg-0);
  }
  .newdir {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line-0);
    background: var(--bg-1);
  }
  .newdir .hint {
    flex-basis: 100%;
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .caret.open {
    transform: rotate(90deg);
  }
  .gbrow .name {
    padding-left: 26px;
  }
  .gbrow .gbedit {
    margin: 2px 10px 2px 26px;
  }
  .bname {
    appearance: none;
    border: 1px solid transparent;
    background: none;
    padding: 3px 6px;
    border-radius: var(--r-1);
    color: var(--fg-0);
    font-size: var(--fs-md, 14px);
    text-align: left;
    cursor: text;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .bname:hover:not(:disabled) {
    background: var(--bg-hover);
  }
  .bname:disabled {
    color: var(--fg-3);
    cursor: default;
  }
  .recover {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 10px;
    margin: 8px 10px;
    padding: 7px 10px;
    border: 1px solid var(--line-1);
    border-left: 2px solid var(--focus);
    border-radius: var(--r-1);
    font-size: var(--fs-xs);
    color: var(--fg-1);
  }
  .acts2 {
    display: flex;
    gap: 6px;
  }
  .row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px 0 0;
    border-bottom: 1px solid var(--line-0);
    font-size: var(--fs-sm);
  }
  .row:hover {
    background: var(--bg-2);
  }
  .row .name {
    appearance: none;
    border: 0;
    background: none;
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 12px;
    color: var(--fg-1);
    text-align: left;
    cursor: pointer;
    font-size: var(--fs-sm);
  }
  .ntext {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .noart {
    flex: none;
    width: 20px;
    height: 20px;
  }
  .row:hover .name {
    color: var(--fg-0);
  }
  .row .meta {
    display: flex;
    align-items: baseline;
    gap: 12px;
    white-space: nowrap;
    font-size: var(--fs-xs);
    color: var(--fg-2);
  }
  .cls {
    color: var(--c-class);
  }
  .lvl {
    width: 38px;
    text-align: right;
  }
  .date {
    width: 78px;
    text-align: right;
  }
  .acts {
    position: absolute;
    top: 50%;
    right: 6px;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 2px;
    padding-left: 18px;
    background: linear-gradient(to right, transparent, var(--bg-2) 14px);
    opacity: 0;
    pointer-events: none;
  }
  .row:hover .acts,
  .acts:focus-within,
  .acts.open {
    opacity: 1;
    pointer-events: auto;
  }
  .row:hover .date,
  .row:has(.acts:focus-within, .acts.open) .date {
    visibility: hidden;
  }
  .act {
    appearance: none;
    border: 0;
    background: none;
    color: var(--fg-3);
    font-size: var(--fs-xs);
    font-family: var(--mono);
    cursor: pointer;
    padding: 2px 4px;
    border-radius: 3px;
  }
  .act:hover {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .act.danger {
    color: var(--red, #e06c75);
  }
  .act.on {
    background: var(--bg-active);
    color: var(--fg-0);
  }
  .reimport {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0 8px 6px;
    padding: 8px 10px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    background: var(--bg-1);
  }
  .rpart {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 4px 14px;
  }
  .rpart .chk {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    color: var(--fg-1);
  }
  .rpart .chk.sub {
    color: var(--fg-2);
  }
  .rgo {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }
  .select.xs,
  .input.xs {
    height: 20px;
    font-size: var(--fs-xs);
  }
  .block {
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .grow {
    flex: 1;
  }
  .current {
    gap: 6px;
  }
  .fld {
    display: grid;
    grid-template-columns: 52px 1fr;
    align-items: center;
    gap: 8px;
  }
  .fld .grow {
    min-width: 0;
  }
  .arow {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .arow .label {
    width: 52px;
    flex: none;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .charform,
  .charhead {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .charform .grow {
    min-width: 0;
  }
  .charlist {
    display: flex;
    flex-direction: column;
    max-height: 220px;
    overflow-y: auto;
    border: 1px solid var(--line-0);
    border-radius: var(--r-2);
  }
  .charrow {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 8px;
    font-size: var(--fs-sm);
  }
  .charrow + .charrow {
    border-top: 1px solid var(--line-0);
  }
  .charrow:hover {
    background: var(--bg-2);
  }
  .cleague {
    margin-left: 6px;
  }
  .noart.small-art {
    width: 18px;
    height: 18px;
  }
  .nobuild {
    flex: none;
    white-space: nowrap;
  }
  .cname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .flash {
    margin: 0 10px;
    padding: 6px 8px;
    border: 1px solid var(--line-1);
    border-radius: var(--r-1);
    font-size: var(--fs-xs);
    color: var(--ok);
  }
  .small {
    font-size: var(--fs-xs);
  }
  .pad {
    padding: 10px 12px;
  }
  .overlay {
    position: fixed;
    inset: 0;
    background: var(--backdrop);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }
  .modal {
    width: 560px;
    max-width: 90vw;
    max-height: 78vh;
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--line-1);
    border-radius: var(--r-2);
    box-shadow: var(--shadow-modal);
  }
  .mhead {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-bottom: 1px solid var(--line-0);
  }
  .mhead .btn {
    margin-left: auto;
  }
  .mbody {
    overflow-y: auto;
    padding: 4px 0 8px;
  }
  .mrow {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px;
  }
  .mrow + .mrow {
    border-top: 1px solid var(--line-0);
  }
  .mtext {
    flex: 1;
    min-width: 0;
  }
  .pmode {
    align-items: flex-start;
    cursor: pointer;
  }
  .pmode input {
    margin: 3px 0 0;
  }
  .pmode .mtext > span {
    display: block;
  }
  .mtitle {
    font-size: var(--fs-sm);
    color: var(--fg-0);
    margin-bottom: 2px;
  }
  .mvariants {
    display: flex;
    flex-direction: column;
    padding: 0 6px;
  }
  .mvar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 3px 6px;
    border-radius: 3px;
  }
  .mvar:hover {
    background: var(--bg-2);
  }
  .mvar .act {
    opacity: 0;
  }
  .mvar:hover .act,
  .mvar .act:focus-visible {
    opacity: 1;
  }
  .mvname {
    flex: 1;
    min-width: 0;
    font-size: var(--fs-sm);
    color: var(--fg-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .mtrunc {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
