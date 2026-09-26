import { invoke } from "@tauri-apps/api/core";
import { untrack } from "svelte";

/** Mirrors pob_engine::EngineStatus. */
export interface EngineStatus {
  state: "booting" | "ready" | "error" | "stopped";
  message: string | null;
  boot_ms: number | null;
  pob_root: string;
  user_dir: string;
}

export interface CallResult<T> {
  result: T;
  elapsed_ms: number;
}

export class EngineError extends Error {
  constructor(
    public method: string,
    message: string,
  ) {
    super(message);
    this.name = "EngineError";
  }
}

/** Telemetry for the status bar: last call, its cost, and running count. */
export const telemetry = $state({
  lastMethod: "",
  lastMs: 0,
  calls: 0,
  inflight: 0,
});

export async function call<T = unknown>(method: string, params?: unknown): Promise<T> {
  // untrack: read-modify-writes of telemetry must never become dependencies of
  // whatever effect happens to be calling the engine (infinite update loops).
  untrack(() => telemetry.inflight++);
  try {
    const r = await invoke<CallResult<T>>("engine_call", { method, params: params ?? null });
    untrack(() => {
      telemetry.lastMethod = method;
      telemetry.lastMs = r.elapsed_ms;
      telemetry.calls++;
    });
    return r.result;
  } catch (e) {
    throw new EngineError(method, typeof e === "string" ? e : String(e));
  } finally {
    untrack(() => telemetry.inflight--);
  }
}

export function status(): Promise<EngineStatus> {
  return invoke<EngineStatus>("engine_status");
}

export interface AppPaths {
  game: "poe1" | "poe2";
  pob_root: string;
  user_dir: string;
  builds_dir: string;
  sync: {
    upstream_commit: string;
    upstream_commit_date: string;
    upstream_version: string;
    pinned_commit: string | null;
    synced_at_unix: number;
    files: number;
    bytes: number;
  } | null;
  open_on_start: string | null;
  chat_allow: string | null;
  chat_log: string | null;
  chat_provider: string | null;
  chat_model: string | null;
  chat_mode: string | null;
  initial_view: string | null;
  chat_open: string | null;
  chat_ask: string | null;
}

export function appPaths(): Promise<AppPaths> {
  return invoke<AppPaths>("app_paths");
}

/** Which game a build file belongs to, by its root element; null if it is not a PoB build. */
export function buildFileGame(path: string): Promise<"poe1" | "poe2" | null> {
  return invoke<"poe1" | "poe2" | null>("build_file_game", { path });
}

export function buildXmlGame(xml: string): Promise<"poe1" | "poe2" | null> {
  return invoke<"poe1" | "poe2" | null>("build_xml_game", { xml });
}

export interface BuildEntry {
  path: string;
  name: string;
  folder: string;
  class_name: string | null;
  ascend_class_name: string | null;
  level: number | null;
  modified: number;
}

export function listBuilds(): Promise<BuildEntry[]> {
  return invoke<BuildEntry[]>("list_builds");
}

export function readTreeJson(version: string): Promise<string> {
  return invoke<string>("read_tree_json", { version });
}

export function readTextFile(path: string): Promise<string> {
  return invoke<string>("read_text_file", { path });
}

export function writeTextFile(path: string, contents: string): Promise<void> {
  return invoke<void>("write_text_file", { path, contents });
}

export interface PoolStatus {
  size: number;
  spawned: number;
  ready: number;
}

export function poolStatus(): Promise<PoolStatus> {
  return invoke<PoolStatus>("pool_status");
}

/** Push the current build to the worker pool in the background. */
export function poolPresync(): Promise<void> {
  return invoke<void>("pool_presync");
}

/** Collect garbage in every engine; for a quiet moment after a scan. */
export function poolTrim(): Promise<void> {
  return invoke<void>("pool_trim");
}

/** Drop the worker engines if the pool has gone idle. Returns how many went. */
export function poolRelease(): Promise<number> {
  return invoke<number>("pool_release");
}

/** Node power scored across the worker pool (falls back to PoB's sequential builder). */
export function powerScanParallel(stat: string | null, maxDepth: number | null): Promise<{ result: TreePower; elapsed_ms: number }> {
  return invoke<{ result: TreePower; elapsed_ms: number }>("power_scan_parallel", { stat, maxDepth });
}

export interface PointPlanPick {
  id: number;
  name: string | null;
  type: string | null;
  /** Points this pick allocates, travel nodes included. */
  cost: number;
  gain: number;
  path: number[];
}

export interface PointPlan {
  stat: string;
  label: string;
  budget: number;
  spent: number;
  total: number;
  picks: PointPlanPick[];
  ms: number;
}

export function planPointsParallel(stat: string, budget: number): Promise<{ result: PointPlan; elapsed_ms: number }> {
  return invoke<{ result: PointPlan; elapsed_ms: number }>("plan_points_parallel", { stat, budget });
}

/** Warm the gem DPS cache for a group across the pool; best effort. */
export function gemDpsParallel(groupIndex: number): Promise<{ result: unknown; elapsed_ms: number }> {
  return invoke<{ result: unknown; elapsed_ms: number }>("gem_dps_parallel", { groupIndex });
}

export function fetchBuildCode(url: string): Promise<{ site: string; code: string }> {
  return invoke<{ site: string; code: string }>("fetch_build_code", { url });
}

export interface MobalyticsVariant {
  id: string;
  name: string;
  /** The Build Planner file as the site produced it. */
  json: string;
  passives: number;
  skills: number;
}

export interface MobalyticsBuild {
  slug: string;
  url: string;
  /** A pobb.in link or a raw build code, when the author attached one. */
  pobCode: string | null;
  variants: MobalyticsVariant[];
}

export function isMobalyticsLink(url: string): boolean {
  return /^https?:\/\/(www\.)?mobalytics\.gg\/poe-2\/builds\/[^/?#]+/i.test(url.trim());
}

export function resolveMobalytics(url: string): Promise<MobalyticsBuild> {
  return invoke<MobalyticsBuild>("mobalytics_resolve", { url });
}

export function sessionInfo(): Promise<{ uncleanExit: boolean; safeMode: boolean }> {
  return invoke<{ uncleanExit: boolean; safeMode: boolean }>("session_info");
}

export function exportDiagnostics(path: string): Promise<void> {
  return invoke<void>("export_diagnostics", { path });
}

export function revealLogs(): Promise<string> {
  return invoke<string>("reveal_logs");
}

export interface GameCharacter {
  name: string;
  realm: string;
  class: string;
  league: string;
  level: number;
  lastLoginTime?: number;
}

/** `account` comes back with GGG's casing. */
export function characterList(realm: string, account: string): Promise<{ account: string; characters: GameCharacter[] }> {
  return invoke<{ account: string; characters: GameCharacter[] }>("character_list", { realm, account });
}

export function characterData(realm: string, account: string, character: string): Promise<{ passives: string; items: string }> {
  return invoke<{ passives: string; items: string }>("character_data", { realm, account, character });
}

export interface NinjaCharacter {
  name: string;
  className: string | null;
  level: number;
  league: string;
  leagueUrl: string;
  updated: string | null;
  isCurrent: boolean;
  /** "listed" when poe.ninja has the build; otherwise its reason: "belowCutoff", "leagueEnded", "inactive", "notFetched" or "unlisted". */
  status: string;
  minLevel: number | null;
}

/** `account` comes back as poe.ninja spells it. */
export function ninjaCharacters(account: string): Promise<{ account: string; characters: NinjaCharacter[] }> {
  return invoke<{ account: string; characters: NinjaCharacter[] }>("ninja_characters", { account });
}

export function ninjaCharacterCode(account: string, character: string, league: string): Promise<string> {
  return invoke<string>("ninja_character_code", { account, character, league });
}

export interface MaxrollPobLink {
  name: string;
  /** The planner the link came from, or the guide text. */
  source: string;
  url: string;
}

export interface MaxrollGuide {
  title: string;
  url: string;
  links: MaxrollPobLink[];
}

export function isMaxrollGuideLink(url: string): boolean {
  return /^https?:\/\/(www\.)?maxroll\.gg\/poe2?\/(build-guides|planner)\/[^/?#]+/i.test(url.trim());
}

/** A Maxroll build guide or planner, resolved to the PoB links it offers. */
export function resolveMaxroll(url: string): Promise<MaxrollGuide> {
  return invoke<MaxrollGuide>("maxroll_resolve", { url });
}

export interface SavedGameBuild {
  name: string;
  path: string;
  replaced: boolean;
}

/** Write Build Planner files flat into the game's folder (or `dir`). */
export function saveGameBuildFiles(files: { name: string; json: string }[], dir?: string): Promise<SavedGameBuild[]> {
  return invoke<SavedGameBuild[]>("save_game_build_files", { dir: dir || null, files });
}

export function renameBuild(path: string, newName: string): Promise<string> {
  return invoke<string>("rename_build", { path, newName });
}

export function moveBuild(path: string, folder: string): Promise<string> {
  return invoke<string>("move_build", { path, folder });
}

export function deleteBuild(path: string): Promise<void> {
  return invoke<void>("delete_build", { path });
}

export function createBuildFolder(folder: string): Promise<void> {
  return invoke<void>("create_build_folder", { folder });
}

export function deleteBuildFolder(folder: string): Promise<void> {
  return invoke<void>("delete_build_folder", { folder });
}

export function listBuildFolders(): Promise<string[]> {
  return invoke<string[]>("list_build_folders");
}

export interface GameBuildList {
  dir: string;
  exists: boolean;
  builds: { path: string; name: string; author: string | null; ascendancy: string | null; modified: number }[];
}

export function listGameBuilds(dir?: string): Promise<GameBuildList> {
  return invoke<GameBuildList>("list_game_builds", { dir: dir || null });
}

/** Set the name and/or author written inside a game Build Planner file; an empty author clears it. The file keeps its own name. */
export function setGameBuildMeta(path: string, meta: { name?: string; author?: string }): Promise<void> {
  return invoke<void>("set_game_build_meta", { path, name: meta.name ?? null, author: meta.author ?? null });
}

export const SHARE_SITES = ["pobb.in", "Maxroll", "poe.ninja", "poe2db.tw", "poedb.tw"] as const;
export type ShareSite = (typeof SHARE_SITES)[number];

/** The sites that host the active game's builds; the host uploads to that game's endpoints. */
export function shareSitesFor(game: "poe1" | "poe2"): readonly ShareSite[] {
  return game === "poe2" ? ["pobb.in", "Maxroll", "poe.ninja", "poe2db.tw"] : ["pobb.in", "Maxroll", "poe.ninja", "poedb.tw"];
}

/** Upload a build code to a sharing site and get the link back. */
export function shareBuildCode(site: ShareSite, code: string): Promise<{ site: string; url: string }> {
  return invoke<{ site: string; url: string }>("share_build_code", { site, code });
}

// ---------------------------------------------------------------------------
// Typed bridge methods (see crates/pob-engine/lua/bridge.lua)
// ---------------------------------------------------------------------------

export interface Points {
  used: number;
  max: number;
  ascUsed: number;
  ascMax: number;
  weaponSet1Used: number;
  weaponSet2Used: number;
  weaponSetMax: number;
  socketsUsed: number;
  requiredLevelText: string | null;
  act: string | null;
}

export interface BuildInfo {
  generation: number;
  name: string;
  file: string | null;
  level: number;
  levelAuto: boolean;
  classId: number;
  className: string;
  ascendClassId: number;
  ascendClassName: string | null;
  /** PoE1's alternate ascendancy; 0 and null on PoE2. */
  secondaryAscendClassId: number;
  secondaryAscendClassName: string | null;
  mainSocketGroup: number;
  treeVersion: string;
  rev: number;
  unsaved: boolean;
  title: string;
  points: Points;
  targetVersion: string;
}

export interface SidebarRow {
  h: number;
  lhs: string | null;
  rhs: string | null;
  breakdown: string | null;
  hasBreakdown: boolean;
  align: string | null;
  /** PoB's output key for the row (`Life`, `TotalDPS`); null for spacers and PoB's own headings. */
  stat: string | null;
  actor: "player" | "minion" | null;
}

export type BreakdownSection =
  | { type: "text"; size: number; lines: string[] }
  | { type: "table"; label: string | null; footer: string | null; cols: { label: string; key: string; right: boolean }[]; rows: Record<string, string>[] }
  | { type: "radius"; radius: number };

export interface CalcCell {
  index: number;
  text: string;
  hasBreakdown: boolean;
}

export interface CalcRow {
  index: number;
  label: string | null;
  /** PoB draws wide tables (the per-damage-type rows) at 12px instead of 16. */
  textSize: number | null;
  cells: CalcCell[];
}

export interface CalcSubSection {
  index: number;
  label: string;
  extra: string | null;
  /** PoB's fixed value-column width for tables; null means one column that takes the rest. */
  colWidth: number | null;
  rows: CalcRow[];
}

export interface CalcSection {
  index: number;
  group: number | null;
  colour: string | null;
  enabled: boolean;
  subSections: CalcSubSection[];
}

export interface Sidebar {
  rows: SidebarRow[];
  warnings: string[];
  rev: number;
}

export interface SocketedJewel {
  nodeId: number;
  itemId: number;
  name: string;
  title: string | null;
  baseName: string | null;
  rarity: string | null;
  radiusIndex: number | null;
  radiusLabel: string | null;
  /** Legion of a timeless-style jewel (vaal, karui, ...), which picks its ring art. */
  conqueror: string | null;
  /** From Nothing and Impossible Escape: the keystone node ids the radius follows, instead of the socket. */
  radiusKeystones: number[] | null;
}

export interface NodeOverride {
  name: string | null;
  icon: string | null;
  /** Glow drawn under the node (a tattoo's, or a mastery's chosen effect). */
  effect: string | null;
  stats: string[];
  overlay: { alloc: string; path: string; unalloc: string } | null;
}

export interface TreeState {
  treeVersion: string;
  classId: number;
  className: string;
  ascendClassId: number;
  ascendClassName: string | null;
  allocatedNodes: number[];
  allocatedNodeCount: number;
  weaponSet1Nodes: number[];
  weaponSet2Nodes: number[];
  /** Counts weapon-set nodes too; use passivePointsSpent against a point budget. */
  pointsUsed: number;
  /** What PoB charges the budget: main-tree nodes plus the larger weapon set. */
  passivePointsSpent: number;
  mainTreePointsUsed: number;
  ascendancyPointsUsed: number;
  secondaryAscendancyPointsUsed: number;
  jewelSocketsUsed: number;
  weaponSet1PointsUsed: number;
  weaponSet2PointsUsed: number;
  weaponSetPointsAvailablePerSet: number;
  characterLevel: number;
  pointsFromLevels: number;
  /** Quest points depend on campaign progress, not level, so the budget is a range. */
  questPointsMin: number;
  questPointsMax: number;
  extraPoints: number;
  pointsAvailableMin: number;
  pointsAvailableMax: number;
  ascendancyPointsAvailable: number;
  /** Nodes whose live content differs from tree.json (switched attributes, ascendancy variants). */
  overrides: Record<string, NodeOverride>;
  sockets: SocketedJewel[];
  /** PoE1 cluster jewel subgraph nodes, generated by PoB from the socketed jewel. */
  dynamicNodes: DynamicNode[];
  /** One per cluster subgraph: its centre and the orbits in use, for the ring art. */
  dynamicGroups: DynamicGroup[];
  rev: number;
}

export interface DynamicGroup {
  x: number;
  y: number;
  orbits: number[];
}

export interface DynamicNode {
  id: number;
  name: string | null;
  type: string | null;
  stats: string[];
  x: number;
  y: number;
  icon: string | null;
  links: number[];
  expansion: boolean;
  allocated: boolean;
}

export interface MasteryEffect {
  effect: number;
  stats: string[];
  /** Node id of another mastery that already holds this effect, if any. */
  takenBy: number | null;
}

export interface TreeClickResult extends Partial<TreeState> {
  needsConfirm?: "class_change";
  needsAttribute?: boolean;
  /** PoE1: the mastery needs an effect chosen before it can be allocated. */
  needsMastery?: boolean;
  name?: string | null;
  effects?: MasteryEffect[];
  selected?: number | null;
  className?: string;
  ascendClassName?: string | null;
  id?: number;
  /** Why the click was refused in this weapon set mode; nothing changed. */
  blocked?: string;
}

export interface NodeInfo extends Record<string, unknown> {
  id: number;
  name: string | null;
  stats: string[];
  masteryEffects: MasteryEffect[];
  masterySelected: number | null;
}

/** Where a tree click allocates: 0 is the main tree, 1 and 2 are the weapon sets. */
export type WeaponSetMode = 0 | 1 | 2;

export interface HoverInfo {
  id: number;
  allocated: boolean;
  path: number[];
  depends: number[];
  cost?: number;
  blocked: string | null;
}

export interface NodeCompare {
  id: number;
  allocated: boolean;
  granted: boolean;
  pathCount: number;
  changes: number;
  lines: (TooltipLine & { head?: boolean })[];
  rev: number;
}

export interface SpecInfo {
  index: number;
  title: string;
  className: string;
  ascendClassName: string | null;
  allocatedNodeCount: number;
  treeVersion: string;
  active: boolean;
}

export interface JewelRadius {
  inner: number;
  outer: number;
  color: string;
  label: string;
}

export interface PowerStat {
  stat: string | null;
  label: string;
}

export interface NodePower {
  s: number | null;
  o: number | null;
  d: number | null;
  p: number | null;
  dist: number | null;
}

export interface PowerReportRow {
  id: number;
  name: string;
  power: number;
  powerStr: string;
  pathPower: number;
  pathPowerStr: string;
  allocated: boolean;
  pathDist: number | null;
  type: string | null;
}

export interface TreePower {
  stat: string | null;
  label: string;
  nodes: Record<string, NodePower>;
  max: { singleStat: number; offence: number; defence: number; offencePerPoint: number; defencePerPoint: number };
  report: PowerReportRow[];
  ms: number;
  rev: number;
}

export interface ClassInfo {
  id: number;
  name: string;
  ascendancies: { id: number; name: string; internalId?: string | null }[];
}

export interface GemInfo {
  index: number;
  nameSpec: string | null;
  name: string | null;
  gemId: string | null;
  skillId: string | null;
  level: number | null;
  maxLevel: number;
  quality: number | null;
  enabled: boolean;
  support: boolean;
  /** PoB colour escape for the gem name (Str/Dex/Int). */
  color: string | null;
  /** Physical socket colour from PoB's gem data. */
  socketColour?: string | null;
  count: number | null;
  errMsg: string | null;
  /** Set when the game hands the skill out (a weapon's default attack, Raise Shield, a unique's skill): says what it comes with. */
  granted: string | null;
}

/** Where an item-, node- or mechanic-granted socket group comes from. */
export interface GrantedBy {
  kind: "item" | "node" | "mechanic";
  item: string | null;
  node: string | null;
  slot: string | null;
  source: string;
}

/** One skill granted by a socket group; selector fields only on the chosen one. */
export interface SkillEntry {
  index: number;
  name: string;
  parts?: { name: string; stages: boolean }[];
  part?: number;
  statSets?: string[];
  statSet?: number;
  hasStages?: boolean;
  stageCount?: number;
  hasMines?: boolean;
  mineCount?: number | null;
  minions?: { id: string; name: string }[];
  minion?: string | null;
  /** Set when this skill picks its minion from the player's library, so the library button belongs here. */
  minionLibrary?: "spectre" | "beast";
  minionSkills?: string[];
  minionSkill?: number;
}

export interface SocketGroup {
  index: number;
  label: string | null;
  displayLabel: string | null;
  enabled: boolean;
  includeInFullDPS: boolean;
  slot: string | null;
  source: string | null;
  /** How many copies of an item-granted group apply; null when the player socketed it. */
  groupCount: number | null;
  grantedBy: GrantedBy | null;
  /** On an item's copy of a skill: the socketed group that carries the same skill and its supports. */
  duplicateOf?: number;
  /** On a socketed group: the item's support-less copy of the same skill. */
  grantedCopy?: number;
  mainActiveSkill: number | null;
  gems: GemInfo[];
  skills: SkillEntry[];
  isMainSkill: boolean;
}

export interface SkillSetInfo {
  id: number;
  title: string;
  active: boolean;
}

export interface Skills {
  socketGroups: SocketGroup[];
  mainSocketGroup: number | null;
  skillSets: SkillSetInfo[];
  activeSkillSet: number | null;
}

export interface GemSearchRow {
  gemId: string;
  name: string;
  support: boolean;
  valid: boolean;
  color: string;
  tags: string | null;
  family: string | null;
  gemType: string | null;
  maxLevel: number;
  tier: number | null;
  legacy: boolean;
  dps?: number;
  dpsDiff?: number;
}

export interface SkillsOptions {
  defaultGemLevel: string;
  defaultGemQuality: number;
  sortGemsByDPS: boolean;
  sortGemsByDPSField: string;
  showSupportGemTypes: string;
  showLegacyGems: boolean;
  /** What the two dropdowns may be set to, straight from PoB's own lists. */
  sortFields: string[];
  supportTypes: string[];
}

/** Which buffs the Calcs tab assumes; the sidebar is always EFFECTIVE. */
export interface CalcMode {
  mode: string;
  modes: string[];
}

export interface TooltipLine {
  size: number;
  text: string;
  center: boolean;
  sep: boolean;
  /** "FONTIN SC" on the lines PoB draws in the game font. */
  font: string | null;
}

/** Rarity art PoB frames a tooltip with; null for plain tooltips. */
export type TooltipHeader = "UNIQUE" | "RARE" | "MAGIC" | "NORMAL" | "RELIC" | "GEM" | null;

export interface Tooltip {
  lines: TooltipLine[];
  header: TooltipHeader;
  runic: boolean;
  uniqueGem: boolean;
  itemArt?: { game: "poe1" | "poe2"; name: string | null; baseName: string | null; rarity: string | null };
}

export type GemKind = "skill" | "spirit" | "support";

export interface GemName {
  name: string;
  gemId: string;
  kind: GemKind;
}


export interface SkillRow {
  group: number;
  skill: string;
  press: "active" | "persistent" | "trigger" | "meta" | "granted";
  supports: number;
  enabled: boolean;
  main: boolean;
}

export interface BuildSummary {
  characterLevel: number;
  className: string;
  ascendancyName: string | null;
  mainSkill: string | null;
  mainSkillGroup: number;
  mainSkillSupports: number;
  activeSkills: number;
  persistentSkills: number;
  triggerSkills: number;
  metaSkills: number;
  skills: SkillRow[];
  pointsUsed: number;
  /** What PoB charges the budget: main-tree nodes plus the larger weapon set. */
  passivePointsSpent: number;
  extraPoints: number;
  pointsAvailableMin: number;
  pointsAvailableMax: number;
  ascendancyPointsUsed: number;
  jewelSocketsUsed: number;
  weaponSetPointsUsed: number;
  life: number;
  energyShield: number;
  mana: number;
  spirit: number;
  spiritReserved: number;
  spiritUnreserved: number;
  charmLimit: number;
  emptyCharms: number;
  charmsEquipped: number;
  charmsActive: number;
  fireResist: number;
  coldResist: number;
  lightningResist: number;
  chaosResist: number;
  str: number;
  dex: number;
  int: number;
  movementSpeedMod: number;
  totalDPS: number;
  keystones: string[];
  keystoneRules: string[];
}

export interface SanityFinding {
  severity: "high" | "medium" | "low";
  area: string;
  message: string;
  fix: string | null;
}

export interface SanityCheck {
  findings: SanityFinding[];
  high: number;
  medium: number;
  low: number;
}

export interface GearOptParams {
  preset?: "balanced" | "defence" | "damage";
  weights?: { dps: number; life: number; ehp: number };
  slots?: string[];
  itemLevel?: number;
  /** Roll within each tier, 0 to 1. */
  range?: number;
  resist?: number;
  chaos?: number;
  moveSpeed?: number;
  titlePrefix?: string;
}

export interface GearOptProgress {
  done: number;
  total: number;
  slot: string | null;
  note: string;
}

export type GearOptStats = Record<string, number>;

export interface GearProposal {
  slot: string;
  base: string;
  type: string;
  title: string;
  replaces: string | null;
  baseReason: string | null;
  implicit: string | null;
  lookFor: string[];
  runes: string[];
  affixes: { slot: "Prefix" | "Suffix"; group: string; modId: string; text: string }[];
  mods: string[];
  requirements: { level: number | null; str: number | null; dex: number | null; int: number | null };
  raw: string;
  evaluations: number;
  output: GearOptStats;
  delta: GearOptStats;
}

export interface GearOptResult {
  preset: string;
  weights: { dps: number; life: number; ehp: number };
  itemLevel: number;
  range: number;
  slots: string[];
  before: GearOptStats;
  after: GearOptStats;
  delta: GearOptStats;
  proposals: GearProposal[];
  skipped: { slot: string; reason: string }[];
  ms: number;
}

export interface SlotInfo {
  slot: string;
  label: string | null;
  itemId: number;
  itemName: string | null;
  itemRarity: string | null;
  nodeId: number | null;
  weaponSet: number | null;
  shown: boolean;
  inactive: boolean;
}

export interface SlotsResponse {
  slots: SlotInfo[];
  activeItemSet: number;
  useSecondWeaponSet: boolean;
}

export interface ItemInfo {
  id: number;
  name: string;
  title: string | null;
  baseName: string | null;
  type: string | null;
  rarity: string | null;
  raw: string;
  corrupted: boolean;
  quality: number | null;
  itemLevel: number | null;
  primarySlot: string | null;
  compatibleSlots: string[];
  equippedSlot: string | null;
  sockets?: ItemSocket[];
  runes?: string[];
}

export interface ItemDbRow {
  name: string;
  rarity: string | null;
  type: string;
  baseName: string | null;
  slot: string | null;
  league: string | null;
  implicits: string[];
  /** Mod lines at the item's current variant selection. */
  mods: string[];
  variants: number;
  /** How many variants the item takes at once (a Megalomaniac takes 3 notables). */
  variantPicks: number;
  /** The first variant names, at most 40. */
  variantNames: string[];
  selectedVariants: string[];
  upgrade: boolean;
}

export interface JewelSocketRow {
  socket: string;
  slot: string;
  nodeId: number;
  item: string | null;
  itemRarity: string | null;
  sinister: boolean;
}

export interface JewelSuggestion {
  name: string;
  item: string;
  base: string | null;
  /** "Socket #n" for a radius jewel, "any" otherwise, "n sockets" for a stack. */
  socket: string;
  slot: string | null;
  replaces: string | null;
  variants: string[];
  mods: string[];
  delta: Record<string, number>;
  score: number;
  raw: string;
  copies?: number;
  note?: string;
  alternatives?: { variants: string[]; score: number; delta: Record<string, number> }[];
}

export interface JewelSuggestions {
  summary: string;
  preset?: string;
  range?: number;
  sockets: JewelSocketRow[];
  baseline?: { Life: number; TotalEHP: number; CombinedDPS: number; Armour: number };
  suggestions: JewelSuggestion[];
  notScored: { name: string; reason: string; mods: string[] }[];
  errors: { name: string; error: string }[];
  evaluations: number;
  ms?: number;
}

export interface JewelSuggestParams {
  preset?: "balanced" | "defence" | "damage";
  sockets?: string[];
  names?: string[];
  range?: number;
  limit?: number;
}

export interface ItemSetInfo {
  id: number;
  title: string;
  active: boolean;
}

export interface CraftBase {
  name: string;
  label: string | null;
  subType: string | null;
}

export interface AffixSeries {
  id: string;
  label: string;
  modIds: string[];
}

export interface AffixSlot {
  index: number;
  modId: string;
  range?: number | null;
  rangeIsTable: boolean;
  label: string | null;
  value: string | null;
  affix: string | null;
  options: AffixSeries[];
  rolls: { seriesId: string; tiers: AffixRollTier[] } | null;
}

export interface AffixRollStep {
  position: number;
  range: number;
  value: string;
}

export interface AffixRollTier {
  modId: string;
  affix: string | null;
  tier: number;
  flipped: boolean;
  steps: AffixRollStep[];
}

export interface ItemAffixes {
  crafted: boolean;
  affixLimit?: number;
  prefixes: AffixSlot[];
  suffixes: AffixSlot[];
}

export interface RuneOption {
  name: string;
  label: string | null;
  lines: string[];
  req: number | null;
  type: string | null;
  limit: number | null;
}

export interface ItemRunes {
  socketCount: number;
  runes: string[];
  options: RuneOption[];
}

export type ItemTarget = { itemId: number; raw?: never; generation?: number } | { raw: string; itemId?: never; generation: number };
export type ItemCustomizationEdit =
  | { operation: "props"; quality?: number; itemLevel?: number; corrupted?: boolean; catalyst?: number; catalystQuality?: number }
  | { operation: "affix"; table: "prefixes" | "suffixes"; index: number; modId: string; range?: number }
  | { operation: "affix"; table: "prefixes" | "suffixes"; index: number; seriesId: string; relativePosition: number }
  | { operation: "rune"; index: number; name: string }
  | { operation: "variant"; picks: number[] }
  | { operation: "shape"; influences?: string[]; sockets?: ItemSocket[]; clusterSkill?: string; clusterNodeCount?: number }
  | { operation: "crucible"; selected: string[] }
  | { operation: "enchant"; line?: string; remove?: boolean; slot: number; skill?: string; source?: string }
  | { operation: "anoint"; nodeId: number | null; slot: number }
  | { operation: "corruption"; modIds?: string[]; ranges?: { index: number; value: number }[] }
  | { operation: "normalize" }
  | { operation: "copy_anoints" | "copy_augments"; sourceSlot?: string }
  | { operation: "rune_sockets"; count: number }
  | { operation: "add_modifier"; text?: string; modId?: string }
  | { operation: "modifier"; section: string; index: number; text?: string; disabled?: boolean; remove?: boolean; range?: number };

export interface ItemCustomization {
  shape: ItemShape;
  crucible: ItemCrucible;
  anoints: AnointInfo;
  corruptions: CorruptionInfo;
  enchantable: boolean;
  raw: string;
  quality: number;
  canQuality: boolean;
  itemLevel: number;
  corrupted: boolean;
  runeSocketLimit: number;
  canCopyAnoints: boolean;
  canCopyAugments: boolean;
  affixes: ItemAffixes;
  runes: ItemRunes;
  variants: ItemVariants;
  catalyst: { usable: boolean; names: string[]; catalyst: number; quality: number };
  modifiers: { section: string; index: number; text: string; disabled: boolean; range?: number | null; parsed: boolean }[];
}

export interface ConfigOption {
  var: string;
  label: string | null;
  type: "check" | "count" | "list" | "text" | "integer" | string | null;
  section: string | null;
  tooltip: string | null;
  defaultState: unknown;
  list?: { val: unknown; label: string }[];
  ifSkill: unknown;
  ifFlag: unknown;
  ifCond: unknown;
  ifMod: unknown;
}

export interface ConfigState {
  config: Record<string, unknown>;
  placeholder: Record<string, unknown>;
  activeConfigSet: number;
  sets: { id: number; title: string | null; active: boolean }[];
}

export interface LoadoutState {
  loadouts: string[];
  active: string | null;
}

export interface AppOptions {
  showThousandsSeparators: boolean;
  thousandsSeparator: string;
  decimalSeparator: string;
  defaultGemQuality: number;
  defaultCharLevel: number;
  defaultItemAffixQuality: number;
}

export interface GameBuildImportResult {
  info: BuildInfo;
  allocated: number;
  requested: number;
  missingPassives: string[];
  skillGroups: number;
  missingSkills: string[];
  gearItems: number;
  gearHints: number;
  warnings: string[];
}

export interface AnointInfo {
  anointable: boolean;
  current: string[];
  slots: number;
  nodes: { id: number; name: string; stats: string[]; recipe: string[]; allocated: boolean }[];
}

export interface CorruptionInfo {
  corruptible: boolean;
  corrupted: boolean;
  enchantNum: number;
  mods: { id: string; label: string; group: string | null }[];
  specialMods: { id: string; label: string; group: string | null }[];
  ranges: { index: number; line: string; current: number }[];
}

export interface SharedItem {
  index: number;
  name: string;
  baseName: string | null;
  rarity: string | null;
  raw: string;
}

export interface PartyBox {
  text: string;
  summary: string;
}

export interface MinionEntry {
  id: string;
  name: string;
  category: string | null;
  recommended: boolean;
}

/** PoB's spectre and beast libraries: the monsters this build owns. */
export interface MinionLibrary {
  kind: string;
  owned: MinionEntry[];
  available: MinionEntry[];
  categories: string[];
  /** PoE1 has no beast library. */
  hasBeasts: boolean;
}

/** PoE1 enchantments: what this item can take and what is on it. */
export interface ItemEnchants {
  available: boolean;
  /** Helmets pick a skill first; other slots go straight to a source. */
  bySkill: boolean;
  skills: string[];
  skill: string | null;
  sources: string[];
  source: string | null;
  lines: string[];
  current: string[];
  slots: number;
}

/** PoE1 crucible trees: five nodes, each holding one mod from the pool. */
export interface Tattoo {
  id: string;
  name: string;
  stats: string[];
  legacy: boolean;
}

/** PoE1 tattoos: what a tree node can be replaced with, and what it carries now. */
export interface NodeTattoos {
  available: boolean;
  options: Tattoo[];
  applied: { id: string | null; name: string | null; stats: string[] } | null;
  nodeName: string;
  count: number;
  limit: number;
}

export interface TimelessJewel {
  id: number;
  label: string;
  name: string;
  seedMin: number;
  seedMax: number;
  step: number;
  conquerors: { id: number; label: string }[];
  /** The stat this jewel pools into one "Total" entry, if it has one. */
  total: string | null;
}

export interface TimelessSocket {
  id: number;
  keystone: string;
  label: string;
  allocated: boolean;
}

export interface TimelessNode {
  id: string;
  name: string;
  stats: string[];
  notable: boolean;
  total: boolean;
}

export interface TimelessRadiusNode {
  id: number;
  name: string;
  notable: boolean;
  keystone: boolean;
  allocated: boolean;
}

export interface TimelessInfo {
  available: boolean;
  jewelType: number;
  jewels: TimelessJewel[];
  sockets: TimelessSocket[];
  nodes: TimelessNode[];
  radius: TimelessRadiusNode[];
  devotion: { id: number; label: string }[];
}

export interface TimelessWant {
  id: string;
  weight?: number;
  weight2?: number;
  minWeight?: number;
}

export interface TimelessSearch {
  jewelType: number;
  socket: number;
  desired: TimelessWant[];
  protect?: string[];
  socketFilter?: boolean;
  socketFilterDistance?: number;
  totalMinWeight?: number;
}

export interface TimelessProgress {
  done: boolean;
  progress: number;
  checked: number;
  found: number;
  total: number;
}

export interface TimelessResult {
  results: {
    seed: number;
    weight: number;
    nodes: { id: string; name: string; weight: number; targets: string[] }[];
  }[];
  found: number;
  checked: number;
  total: number;
  jewelType: number;
  jewelName: string;
  socket: number;
  desired: { id: string; name: string; order: number }[];
}

export interface CompareBuild {
  index: number;
  label: string;
  className: string | null;
  ascendClassName: string | null;
  level: number;
  active: boolean;
}

export interface CompareStatRow {
  stat: string;
  label: string;
  mine: number | null;
  theirs: number | null;
  mineText: string | null;
  theirsText: string | null;
  delta: number | null;
  deltaText: string | null;
  percent: number | null;
  /** Null when the two sides match. */
  better: boolean | null;
  same: boolean;
}

export interface CompareSummary {
  rows: CompareStatRow[];
  mine: { label: string; level: number };
  theirs: { label: string; className: string | null; ascendClassName: string | null; level: number };
}

export interface CompareLoadout {
  specs: { index: number; title: string; nodes: number; active: boolean }[];
  itemSets: { id: number; title: string; active: boolean }[];
  skillSets: { id: number; title: string; active: boolean }[];
  socketGroups: { index: number; label: string; active: boolean }[];
}

export type BuySimilarTarget = { itemId: number } | { slot: string; side: "mine" | "theirs" };

export interface BuySimilarInfo {
  name: string;
  unique: boolean;
  category: string;
  baseName: string | null;
  realms: string[];
  listed: string[];
  defences: { label: string; value: number }[];
  /** `searchable` is false when the trade site has no filter for the mod; `ranged` rows take min/max. */
  mods: { lines: string[]; type: string; searchable: boolean; value: number | string | null; ranged: boolean }[];
}

export interface BuySimilarChoices {
  realm: string;
  league: string;
  /** 1-based index into `listed`. */
  listed: number;
  baseType: boolean;
  ilvlMin: number | null;
  ilvlMax: number | null;
  defences: { checked: boolean; min: number | null; max: number | null }[];
  mods: { checked: boolean; min: number | null; max: number | null }[];
}

export interface CompareItemRow {
  slot: string;
  label: string | null;
  mine: { name: string; rarity: string | null } | null;
  theirs: { name: string; rarity: string | null } | null;
  same: boolean;
}

export interface CompareGem {
  name: string;
  level: number;
  quality: number;
  enabled: boolean;
}

export interface CompareSkillGroup {
  label: string;
  slot: string | null;
  enabled: boolean;
  gems: CompareGem[];
}

export interface CompareSkillRow {
  index: number;
  mine: CompareSkillGroup | null;
  theirs: CompareSkillGroup | null;
  same: boolean;
}

export interface CompareConfigRow {
  var: string;
  label: string;
  mine: string | number | boolean | null;
  theirs: string | number | boolean | null;
  same: boolean;
}

export interface CompareTree {
  mine: { nodes: number; title: string; className: string };
  theirs: { nodes: number; title: string; className: string };
  gained: number[];
  lost: number[];
  keystonesGained: string[];
  keystonesLost: string[];
}

export interface ItemCrucible {
  available: boolean;
  nodes: { id: string; label: string; tier: number; type: string | null }[][];
  /** One entry per node; an empty string means the node is clear. */
  selected: string[];
  count: number;
}

export interface ItemSocket {
  colour: string;
  group: number;
}

/** The PoE1-only shape of an item: influence, sockets and cluster jewel settings. */
export interface ItemShape {
  canBeInfluenced: boolean;
  influences: { key: string; name: string; on: boolean }[];
  sockets: ItemSocket[];
  socketLimit: number;
  colours: string[];
  abyssalSocketCount: number;
  cluster: {
    skills: { id: string; name: string }[];
    skill: string | null;
    nodeCount: number;
    minNodes: number;
    maxNodes: number;
  } | null;
}

export interface ItemVariants {
  names: string[];
  /** A 1-based index into `names` per pick; a Watcher's Eye takes two or three. */
  picks: number[];
}

export interface PartyState {
  partyMemberStats: PartyBox;
  auras: PartyBox;
  warcries: PartyBox;
  links: PartyBox;
  enemyConditions: PartyBox;
  enemyMods: PartyBox;
  curses: PartyBox;
  enableExportBuffs: boolean;
}

export type PartyKind = "partyMemberStats" | "auras" | "warcries" | "links" | "enemyConditions" | "enemyMods" | "curses";

export interface CustomModBlock {
  index: number;
  title: string | null;
  enabled: boolean;
  text: string;
  lines: { text: string; status: "ok" | "partial" | "none" | "empty" }[];
}

export const engine = {
  version: () =>
    call<{
      game: "poe1" | "poe2";
      pobVersion: string;
      treeVersions: string[];
      latestTreeVersion: string;
      userPath: string;
      buildPath: string;
    }>("version"),
  newBuild: (name?: string) => call<BuildInfo>("new_build", { name }),
  loadBuildXml: (xml: string, name?: string, path?: string) => call<BuildInfo>("load_build_xml", { xml, name, path }),
  loadBuildCode: (code: string, name?: string) => call<BuildInfo>("load_build_code", { code, name }),
  /** Which game a share code belongs to, without loading it. */
  codeGame: (code: string) => call<{ game: "poe1" | "poe2" | null }>("code_game", { code }),
  loadBuildFile: (path: string) => call<BuildInfo>("load_build_file", { path }),
  saveBuildXml: () => call<{ xml: string }>("save_build_xml"),
  saveBuildCode: () => call<{ code: string }>("save_build_code"),
  saveBuildFile: (path?: string) => call<{ ok: boolean; path: string; unsaved: boolean }>("save_build_file", path ? { path } : undefined),
  /** Rename the open build; a saved build's file moves with it. */
  setBuildName: (name: string) => call<BuildInfo>("set_build_name", { name }),
  getBuild: () => call<BuildInfo>("get_build"),
  getSidebar: () => call<Sidebar>("get_sidebar"),
  sidebarBreakdown: (rowIndex: number) => call<{ sections: BreakdownSection[]; rev: number }>("sidebar_breakdown", { rowIndex }),
  calcSections: (actor?: "player" | "minion") => call<{ sections: CalcSection[]; rev: number }>("calc_sections", { actor }),
  calcCellBreakdown: (ref: { section: number; sub: number; row: number; col: number; actor?: string }) =>
    call<{ sections: BreakdownSection[]; rev: number }>("calc_cell_breakdown", ref),
  configVisibility: () => call<{ visibility: Record<string, boolean>; rev: number }>("config_visibility"),
  getStats: (fields?: string[]) => call<{ stats: Record<string, number | string | boolean>; rev: number }>("get_stats", fields ? { fields } : undefined),
  setLevel: (level: number) => call<BuildInfo>("set_level", { level }),
  setLevelAuto: (auto: boolean) => call<BuildInfo>("set_level_auto", { auto }),
  listClasses: () => call<{ classes: ClassInfo[]; secondaryAscendancies: { id: number; name: string }[] }>("list_classes"),
  selectClass: (classId?: number, ascendClassId?: number, secondaryAscendClassId?: number) =>
    call<BuildInfo>("select_class", { classId, ascendClassId, secondaryAscendClassId }),
  getTreeState: () => call<TreeState>("get_tree_state"),
  allocNode: (id: number) => call<TreeState>("alloc_node", { id }),
  deallocNode: (id: number) => call<TreeState>("dealloc_node", { id }),
  nodePath: (id: number) => call<{ id: number; path: number[]; cost: number; allocated: boolean }>("node_path", { id }),
  nodeInfo: (id: number) => call<NodeInfo>("node_info", { id }),
  selectMastery: (id: number, effect: number) => call<TreeState>("select_mastery", { id, effect }),
  nodeHover: (id: number, weaponSet: WeaponSetMode = 0) => call<HoverInfo>("node_hover", { id, weaponSet }),
  nodeCompare: (id: number, opts?: { weaponSet?: WeaponSetMode; path?: number[] }) =>
    call<NodeCompare>("node_compare", { id, ...opts }),
  treeClick: (id: number, opts?: { attribute?: number; confirm?: "reset" | "connect"; weaponSet?: WeaponSetMode }) =>
    call<TreeClickResult>("tree_click", { id, ...opts }),
  switchAttribute: (id: number, attribute: number) => call<TreeState>("switch_attribute", { id, attribute }),
  treeUndo: () => call<TreeState>("tree_undo"),
  treeRedo: () => call<TreeState>("tree_redo"),
  exportTreeUrl: () => call<{ url: string }>("export_tree_url"),
  importTreeUrl: (url: string) => call<TreeState>("import_tree_url", { url }),
  jewelRadii: () => call<{ radii: JewelRadius[] }>("jewel_radii"),
  specAlloc: (index: number) => call<{ index: number; allocatedNodes: number[] }>("spec_alloc", { index }),
  allocTrace: (ids: number[], weaponSet: WeaponSetMode = 0) => call<TreeState>("alloc_trace", { ids, weaponSet }),
  socketNodes: (id: number, radiusIndex: number) =>
    call<{ id: number; radiusIndex: number | null; nodes: number[] }>("socket_nodes", { id, radiusIndex }),
  powerStats: () => call<{ stats: PowerStat[] }>("power_stats"),
  treePower: (stat: string | null, maxDepth: number | null) => call<TreePower>("tree_power", { stat, maxDepth }),
  treePowerStart: (stat: string | null, maxDepth: number | null) =>
    call<{ done: boolean; progress: number }>("tree_power_start", { stat, maxDepth }),
  treePowerStep: (budgetMs = 150) => call<{ done: boolean; progress: number }>("tree_power_step", { budgetMs }),
  treePowerResult: () => call<TreePower>("tree_power_result"),
  listSpecs: () => call<{ specs: SpecInfo[]; activeSpec: number }>("list_specs"),
  selectSpec: (index: number) => call<{ specs: SpecInfo[]; activeSpec: number }>("select_spec", { index }),
  createSpec: (title?: string) => call<{ specs: SpecInfo[]; activeSpec: number }>("create_spec", { title }),
  copySpec: (index?: number, title?: string) => call<{ specs: SpecInfo[]; activeSpec: number }>("copy_spec", { index, title }),
  renameSpec: (index: number, title: string) => call<{ specs: SpecInfo[]; activeSpec: number }>("rename_spec", { index, title }),
  deleteSpec: (index: number) => call<{ specs: SpecInfo[]; activeSpec: number }>("delete_spec", { index }),
  convertTree: (opts?: { all?: boolean; replace?: boolean; version?: string }) =>
    call<{ specs: SpecInfo[]; activeSpec: number }>("convert_tree", opts ?? {}),
  getSkills: () => call<Skills>("get_skills"),
  setMainSkill: (index: number) => call<Skills>("set_main_skill", { index }),
  setMainSkillOptions: (
    groupIndex: number,
    patch: { mainActiveSkill?: number; part?: number; statSet?: number; stageCount?: number; mineCount?: number; minionId?: string; minionSkill?: number },
  ) => call<Skills>("set_main_skill_options", { groupIndex, ...patch }),
  addSocketGroup: (label?: string) => call<{ ok: boolean; groupIndex: number }>("add_socket_group", { label }),
  removeSocketGroup: (index: number) => call<Skills>("remove_socket_group", { index }),
  moveSocketGroup: (from: number, to: number) => call<Skills>("move_socket_group", { from, to }),
  addGem: (groupIndex: number, gemId: string, level?: number, quality?: number) =>
    call<Skills>("add_gem", { groupIndex, gemId, level, quality }),
  removeGem: (groupIndex: number, gemIndex: number) => call<Skills>("remove_gem", { groupIndex, gemIndex }),
  moveGem: (groupIndex: number, from: number, to: number) => call<Skills>("move_gem", { groupIndex, from, to }),
  gemSearch: (opts: { groupIndex?: number; query?: string; onlySupports?: boolean; limit?: number; sortByDps?: boolean }) =>
    call<{ gems: GemSearchRow[]; total: number; truncated: boolean; baseDps: number | null }>("gem_search", opts),
  getSkillsOptions: () => call<SkillsOptions>("get_skills_options"),
  setSkillsOptions: (patch: Partial<SkillsOptions>) => call<SkillsOptions>("set_skills_options", patch),
  calcMode: (mode?: string) => call<CalcMode>("calc_mode", mode ? { mode } : {}),
  copySocketGroup: (index: number) => call<{ text: string }>("copy_socket_group", { index }),
  pasteSocketGroup: (text: string) => call<Skills>("paste_socket_group", { text }),
  gemTooltip: (groupIndex: number, gemIndex: number) => call<Tooltip>("gem_tooltip", { groupIndex, gemIndex }),
  /** The same tooltip for any gem, socketed or not, at the build's default gem level. */
  gemTooltipById: (gemId: string) => call<Tooltip>("gem_tooltip", { gemId }),
  /** Every gem name with its kind, for highlighting names in prose. */
  gemNames: () => call<{ gems: GemName[] }>("gem_names"),
  selectSkillSet: (id: number) => call<Skills>("select_skill_set", { id }),
  createSkillSet: (title?: string) => call<Skills>("create_skill_set", { title }),
  copySkillSet: (id?: number, title?: string) => call<Skills>("copy_skill_set", { id, title }),
  renameSkillSet: (id: number, title: string) => call<Skills>("rename_skill_set", { id, title }),
  deleteSkillSet: (id: number) => call<Skills>("delete_skill_set", { id }),
  setSocketGroup: (index: number, patch: Partial<Pick<SocketGroup, "enabled" | "includeInFullDPS" | "label" | "slot" | "mainActiveSkill" | "groupCount">>) =>
    call<Skills>("set_socket_group", { index, ...patch }),
  setGem: (groupIndex: number, gemIndex: number, patch: Partial<Pick<GemInfo, "level" | "quality" | "enabled" | "count">>) =>
    call<Skills>("set_gem", { groupIndex, gemIndex, ...patch }),
  listSlots: () => call<SlotsResponse>("list_slots"),
  getItems: () => call<{ items: ItemInfo[] }>("get_items"),
  equipItemRaw: (text: string, slot?: string, generation?: number) => call<{ ok: boolean; itemId: number; slot: string; itemName: string }>("equip_item_raw", { text, slot, generation }),
  equipItem: (slot: string, itemId: number) => call<SlotsResponse>("equip_item", { slot, itemId }),
  deleteItem: (itemId: number) => call<{ items: ItemInfo[] }>("delete_item", { itemId }),
  itemDbList: (opts: { db: "unique" | "rare"; query?: string; type?: string; limit?: number; offset?: number }) =>
    call<{ items: ItemDbRow[]; total: number; offset: number; types: { type: string; count: number }[] }>("item_db_list", opts),
  statDifferences: (show?: boolean) => call<{ show: boolean }>("stat_differences", show === undefined ? undefined : { show }),
  itemTooltip: (opts: { itemId?: number; db?: "unique" | "rare"; name?: string; raw?: string; slotName?: string | false }) =>
    call<Tooltip & { rarity: string | null }>("item_tooltip", opts),
  prepareItemPreview: (raw: string, generation: number, normalise: boolean) =>
    call<{ raw?: string }>("item_prepare_preview", { raw, generation, normalise }),
  itemPreview: (raw: string, generation: number) =>
    call<{ tooltip: Tooltip; slots: { slot: string; label: string }[]; generation: number; rev: number }>("item_preview", { raw, generation }),
  itemCustomization: (target: ItemTarget) => call<ItemCustomization>("item_customization", target),
  customizeItem: (target: ItemTarget, edit: ItemCustomizationEdit) => call<ItemCustomization>("item_customize", { ...target, ...edit }),
  itemModifierOptions: (target: ItemTarget, source: "Prefix" | "Suffix", query: string) =>
    call<{ options: { id: string; label: string; level: number }[]; total: number }>("item_modifier_options", { ...target, source, query }),
  /** `variants`: one entry per pick, a variant's name, a substring of it, or its index. */
  itemDbEquip: (db: "unique" | "rare", name: string, slotName?: string, variants?: (string | number)[]) =>
    call<{ ok: boolean; itemId: number; slot: string; itemName: string; variants: string[]; mods: string[] }>("item_db_equip", { db, name, slotName, variants }),
  itemRaw: (itemId: number) => call<{ raw: string }>("item_raw", { itemId }),
  itemEdit: (text: string, itemId?: number, generation?: number) => call<{ ok: boolean; itemId: number; name: string }>("item_edit", { text, itemId, generation }),
  setWeaponSet: (set: 1 | 2) => call<SlotsResponse>("set_weapon_set", { set }),
  craftBases: () => call<{ types: string[]; bases: Record<string, CraftBase[]> }>("craft_bases"),
  craftItem: (opts: { type: string; baseName: string; rarity?: string; title?: string; equip?: boolean }) =>
    call<{ ok: boolean; itemId: number; name: string; crafted: boolean }>("craft_item", opts),
  itemAffixes: (itemId: number) => call<ItemAffixes>("item_affixes", { itemId }),
  itemAffixRolls: (target: ItemTarget, table: "prefixes" | "suffixes", index: number, seriesId: string) =>
    call<{ tiers: AffixRollTier[] }>("item_affix_rolls", { ...target, table, index, seriesId }),
  itemRunes: (itemId: number) => call<ItemRunes>("item_runes", { itemId }),
  setItemRune: (itemId: number, index: number, name: string) => call<ItemRunes>("set_item_rune", { itemId, index, name }),
  setItemProps: (itemId: number, patch: { quality?: number; itemLevel?: number; corrupted?: boolean; catalyst?: number; catalystQuality?: number }) =>
    call<{ ok: boolean }>("set_item_props", { itemId, ...patch }),
  itemVariants: (itemId: number) => call<ItemVariants>("item_variants", { itemId }),
  setItemVariant: (itemId: number, picks: number[]) => call<ItemVariants>("set_item_variant", { itemId, picks }),
  listItemSets: () => call<{ itemSets: ItemSetInfo[]; activeItemSet: number }>("list_item_sets"),
  selectItemSet: (id: number) => call<{ itemSets: ItemSetInfo[]; activeItemSet: number }>("select_item_set", { id }),
  createItemSet: (title?: string) => call<{ itemSets: ItemSetInfo[]; activeItemSet: number }>("create_item_set", { title }),
  copyItemSet: (id?: number, title?: string) => call<{ itemSets: ItemSetInfo[]; activeItemSet: number }>("copy_item_set", { id, title }),
  renameItemSet: (id: number, title: string) => call<{ itemSets: ItemSetInfo[]; activeItemSet: number }>("rename_item_set", { id, title }),
  deleteItemSet: (id: number) => call<{ itemSets: ItemSetInfo[]; activeItemSet: number }>("delete_item_set", { id }),
  listConfigOptions: () => call<{ options: ConfigOption[] }>("list_config_options"),
  getConfig: () => call<ConfigState>("get_config"),
  setConfig: (v: string, value: unknown) => call<ConfigState>("set_config", { var: v, value }),
  selectConfigSet: (id: number) => call<ConfigState>("select_config_set", { id }),
  createConfigSet: (title?: string) => call<ConfigState>("create_config_set", title ? { title } : undefined),
  copyConfigSet: (id?: number, title?: string) => call<ConfigState>("copy_config_set", { id, title }),
  renameConfigSet: (id: number, title: string) => call<ConfigState>("rename_config_set", { id, title }),
  deleteConfigSet: (id: number) => call<ConfigState>("delete_config_set", { id }),
  getLoadouts: () => call<LoadoutState>("get_loadouts"),
  selectLoadout: (name: string) => call<LoadoutState>("select_loadout", { name }),
  newLoadout: (name: string) => call<LoadoutState>("new_loadout", { name }),
  copyLoadout: (source: string, name: string) => call<LoadoutState>("copy_loadout", { source, name }),
  renameLoadout: (name: string, newName: string) => call<LoadoutState>("rename_loadout", { name, newName }),
  deleteLoadout: (name: string) => call<LoadoutState>("delete_loadout", { name }),
  catalystInfo: (itemId: number) => call<{ usable: boolean; names: string[]; catalyst: number; quality: number }>("catalyst_info", { itemId }),
  itemAnoints: (target: number | ItemTarget, withNodes = false) => call<AnointInfo>("item_anoints", { ...(typeof target === "number" ? { itemId: target } : target), withNodes }),
  setItemAnoint: (itemId: number, nodeId: number | null, slot?: number) => call<AnointInfo>("set_item_anoint", { itemId, nodeId, slot }),
  itemCorruptions: (itemId: number) => call<CorruptionInfo>("item_corruptions", { itemId }),
  corruptItem: (p: { itemId: number; modIds?: string[]; ranges?: { index: number; value: number }[] }) => call<{ ok: boolean }>("corrupt_item", p),
  getSharedItems: () => call<{ items: SharedItem[] }>("get_shared_items"),
  addSharedItem: (p: { itemId?: number; raw?: string }) => call<{ items: SharedItem[] }>("add_shared_item", p),
  removeSharedItem: (index: number) => call<{ items: SharedItem[] }>("remove_shared_item", { index }),
  equipSharedItem: (index: number, slotName?: string) => call<unknown>("equip_shared_item", { index, slotName }),
  tradeSearchStart: (p: {
    slotName: string;
    statWeights?: { stat: string; weightMult: number }[];
    includeCorrupted?: boolean;
    includeRunes?: boolean;
    includeMirrored?: boolean;
    maxLevel?: number;
    sockets?: number;
    jewelType?: string;
    status?: string;
  }) => call<{ started: boolean }>("trade_search_start", p),
  tradeSearchStep: (steps?: number) => call<{ done: boolean }>("trade_search_step", { steps }),
  tradeSearchResult: () => call<{ query: string }>("trade_search_result"),
  tradeStatusOptions: () => call<{ options: { id: string; label: string }[] }>("trade_status_options"),
  tradeLeagues: () => call<{ leagues: { id: string; text: string; realm: string | null }[] }>("trade_leagues"),
  importGameBuild: (json: string, name?: string) => call<GameBuildImportResult>("import_game_build", { json, name }),
  /** PoE1 only. */
  importCharacter: (p: { character: GameCharacter; passives: string; items: string; name?: string }) =>
    call<BuildInfo>("import_character", p),
  exportGameBuild: (meta?: { author?: string; link?: string; description?: string }) =>
    call<{ json: string; name: string; passives: number; skills: number; gear: number }>("export_game_build", meta ?? {}),
  getParty: () => call<PartyState>("get_party"),
  setPartyText: (kind: PartyKind, text: string) => call<PartyState>("set_party_text", { kind, text }),
  partyImport: (p: { code?: string; xml?: string; append?: boolean; only?: string }) => call<PartyState>("party_import", p),
  itemEnchants: (target: number | ItemTarget, skill?: string, source?: string) => call<ItemEnchants>("item_enchants", { ...(typeof target === "number" ? { itemId: target } : target), skill, source }),
  setItemEnchant: (itemId: number, p: { line?: string; remove?: boolean; slot?: number; skill?: string; source?: string }) =>
    call<ItemEnchants>("set_item_enchant", { itemId, ...p }),
  compareList: () => call<{ entries: CompareBuild[]; active: number }>("compare_list"),
  compareAdd: (p: { xml?: string; code?: string; path?: string; label?: string }) =>
    call<{ entries: CompareBuild[]; active: number }>("compare_add", p),
  compareSelect: (index: number) => call<{ entries: CompareBuild[]; active: number }>("compare_select", { index }),
  compareRemove: (index?: number) => call<{ entries: CompareBuild[]; active: number }>("compare_remove", { index }),
  compareClear: () => call<{ entries: CompareBuild[]; active: number }>("compare_clear"),
  compareSummary: (onlyDifferences?: boolean) => call<CompareSummary>("compare_summary", { onlyDifferences }),
  compareLoadouts: () => call<{ mine: CompareLoadout; theirs: CompareLoadout }>("compare_loadouts"),
  compareSetLoadout: (p: { spec?: number; itemSet?: number; skillSet?: number; socketGroup?: number }) =>
    call<{ mine: CompareLoadout; theirs: CompareLoadout }>("compare_set_loadout", p),
  compareItems: (onlyDifferences?: boolean) => call<{ rows: CompareItemRow[] }>("compare_items", { onlyDifferences }),
  compareItemText: (slot: string, side: "mine" | "theirs") =>
    call<{ text: string | null; name: string | null }>("compare_item_text", { slot, side }),
  compareCopyItem: (slot: string) => call<{ ok: boolean; slot: string; itemName: string }>("compare_copy_item", { slot }),
  buySimilarInfo: (target: BuySimilarTarget) => call<BuySimilarInfo>("buy_similar_info", target),
  buySimilarUrl: (p: BuySimilarTarget & BuySimilarChoices) => call<{ url: string }>("buy_similar_url", p),
  compareSkills: (onlyDifferences?: boolean) => call<{ rows: CompareSkillRow[] }>("compare_skills", { onlyDifferences }),
  compareConfig: (onlyDifferences?: boolean) => call<{ rows: CompareConfigRow[] }>("compare_config", { onlyDifferences }),
  compareTree: () => call<CompareTree>("compare_tree"),
  compareCopyTree: () => call<{ ok: boolean; index: number; title: string }>("compare_copy_tree"),
  timelessInfo: (jewelType?: number, socket?: number) => call<TimelessInfo>("timeless_info", { jewelType, socket }),
  timelessSearchStart: (p: TimelessSearch) => call<TimelessProgress>("timeless_search_start", p),
  timelessSearchStep: (budgetMs?: number) => call<TimelessProgress>("timeless_search_step", { budgetMs }),
  timelessSearchResult: (limit?: number) => call<TimelessResult>("timeless_search_result", { limit }),
  timelessTradeUrl: (p: { jewelType: number; seeds: number[]; conqueror?: number; devotion?: number[]; league?: string; realm?: string; status?: string }) =>
    call<{ url: string; seeds: number; jewelType: number }>("timeless_trade_url", p),
  nodeTattoos: (node: number, legacy?: boolean) => call<NodeTattoos>("node_tattoos", { node, legacy }),
  setNodeTattoo: (node: number, p: { tattoo?: string; remove?: boolean; legacy?: boolean }) => call<NodeTattoos>("set_node_tattoo", { node, ...p }),
  itemCrucible: (itemId: number) => call<ItemCrucible>("item_crucible", { itemId }),
  setItemCrucible: (itemId: number, selected: string[]) => call<ItemCrucible>("set_item_crucible", { itemId, selected }),
  itemShape: (itemId: number) => call<ItemShape>("item_shape", { itemId }),
  setItemShape: (
    itemId: number,
    patch: { influences?: string[]; sockets?: ItemSocket[]; clusterSkill?: string; clusterNodeCount?: number },
  ) => call<ItemShape>("set_item_shape", { itemId, ...patch }),
  minionLibrary: (kind?: string) => call<MinionLibrary>("minion_library", { kind }),
  setMinionLibrary: (kind: string, ids: string[]) => call<MinionLibrary>("set_minion_library", { kind, ids }),
  partyClear: () => call<PartyState>("party_clear"),
  partyDisable: () => call<PartyState>("party_disable"),
  partyRebuild: () => call<PartyState>("party_rebuild"),
  partySetExport: (enabled: boolean) => call<PartyState>("party_set_export", { enabled }),
  getAppOptions: () => call<{ options: AppOptions }>("get_app_options"),
  setAppOptions: (options: Partial<AppOptions>) => call<{ options: AppOptions }>("set_app_options", options),
  getCustomMods: () => call<{ blocks: CustomModBlock[] }>("get_custom_mods"),
  setCustomModBlock: (index: number, patch: { title?: string; enabled?: boolean; text?: string }) =>
    call<{ blocks: CustomModBlock[] }>("set_custom_mod_block", { index, ...patch }),
  addCustomModBlock: (title?: string) => call<{ blocks: CustomModBlock[] }>("add_custom_mod_block", title ? { title } : undefined),
  deleteCustomModBlock: (index: number) => call<{ blocks: CustomModBlock[] }>("delete_custom_mod_block", { index }),
  customModBrowser: () => call<{ mods: { text: string; sources: string[] }[] }>("custom_mod_browser"),
  getNotes: () => call<{ text: string }>("get_notes"),
  setNotes: (text: string) => call<{ ok: boolean }>("set_notes", { text }),
  takeClipboard: () => call<{ text: string | null }>("take_clipboard"),
  // Deterministic build tools (no assistant needed)
  buildSummary: () => call<BuildSummary>("build_summary"),
  sanityCheck: () => call<SanityCheck>("sanity_check"),
  gearOptStart: (p: GearOptParams) => call<{ done: boolean; progress: GearOptProgress }>("gear_opt_start", p),
  gearOptStep: (budgetMs = 150) => call<{ done: boolean; progress: GearOptProgress }>("gear_opt_step", { budgetMs }),
  gearOptResult: () => call<GearOptResult>("gear_opt_result"),
  /** Every unique jewel scored in every allocated socket, one engine; the assistant's tool runs it across the pool. */
  suggestUniqueJewels: (p: JewelSuggestParams = {}) => call<JewelSuggestions>("suggest_unique_jewels", p),
};
