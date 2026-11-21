# Using PoB Redux

How the app the user is in works: where each feature is, what the buttons are called, and the
shortcuts. Read this for any question about the app itself. PoB Redux runs Path of Building's
calculations, but its layout, menus and buttons differ from the original Path of Building, so never
describe the original's interface as this app's. Each message says which tab is open.

Labels below are the English ones. The user may run the app in another language. "Ctrl" is Cmd on
macOS.

## Window layout

- **Title bar**, left to right: the sidebar toggle (Ctrl+B), the **Game** dropdown (POE1 or POE2;
  switching restarts the engine and asks first if the build has unsaved changes), the tabs,
  **Search** (Ctrl+K), the **Settings** gear (Ctrl+,), and the window buttons.
- **Sidebar** on the left: build name (click to rename; a dot means unsaved, Ctrl+S saves), the
  loadout dropdown and a menu with New, Copy, Rename and Delete loadout, the main-skill switcher,
  then stat cards (Offence, Skill, Attributes, Resources, Mitigation, Resistances, Misc, Full DPS,
  Warnings, Not counted). Cards collapse, can be dragged or moved with Alt+Up/Down, and "Reset
  order" restores them. Hover a stat for its breakdown; click to pin the breakdown.
- **Status bar** at the bottom: engine state, worker count, PoB version, the MCP port when the
  server is on, the latest error or notice, the **Assistant** icon (Ctrl+L) and a Discord link.
- **Search** (Ctrl+K) finds commands, passives, items, skills, config options, calculations and
  saved builds. Its last entry sends the typed text to the assistant.

## Tabs

All tabs exist in both games. Everything except Builds is disabled until a build is open.
Ctrl+1 to Ctrl+9 open Builds, Tree, Skills, Items, Calcs, Config, Notes, Party and Optimize.
Overview and Compare have no shortcut. Ctrl+Tab cycles tabs.

Any tab except Builds can move to its own window: drag it out of the title bar and drop it, or
right-click it and choose **Open in new window**. The window shows only that tab, without the
sidebar, and stays in step with every other window. Its tab in the main window shows a small arrow;
clicking it brings the window forward, and right-click offers **Show window** and **Dock in the main
window**. The window's dock button (or closing it) puts the tab back. Torn-off windows reopen where
they were on the next start. In a torn-off window Ctrl+S saves and Ctrl+Z and Ctrl+Y undo and redo
the tree.

### Builds

- Left: the build list. "Filter builds…" (Ctrl+F), sort, **Open file…** (.xml, or a PoE2 `.build`
  planner file), **New folder**, **New build** (Class, Ascendancy, Level, then **Start**).
  "Recently opened", then "Path of Building builds": the same folders the original PoB uses
  (Documents/Path of Building (PoE2)/Builds and Documents/Path of Building/Builds), with
  **Browse…**, **Show in folder** and **Reset**. Each build has buttons to rename it, move it to a
  folder, or delete it.
- PoE2 only: "Build Planner files (.build)" lists the game's own planner files. Clicking one opens
  it as a new build and leaves the file unchanged.
- "Unsaved session" offers the last session's build with **Restore**.
- Right, **Path of Exile account**: **Sign in with Path of Exile** (finishes in the browser).
- Right, **Import**: paste a share code, PoB XML or a link and click **Import**, or **Paste and
  import**. Links from pobb.in, pob.codes, Maxroll, Mobalytics, poe.ninja, poe2db, poedb, Pastebin
  and Rentry work. A Maxroll guide with several PoB links asks which one; a Mobalytics build offers
  its PoB code and its Build Planner files.
- Right, **Character** (needs sign-in): **List my characters**, then per character **import**, or
  **update open build** with checkboxes for the tree, items and skills. Only the signed-in
  account's own characters are listed. For anyone else's character, open it on poe.ninja and click
  **Open in Path of Building**, or copy the code under Import Code for Path of Building and paste
  it into Import.
- Right, **Current build**: rename, author, **Save** (Ctrl+S), **Save as…** (Ctrl+Shift+S), **Copy
  code**, **Link** (uploads to the chosen site and copies the address), **XML…**, and in PoE2
  **.build file…**, which saves to the game's Build Planner.
- `pob://` and `pob2://` links (the "Open in Path of Building" buttons on build sites) open the
  build in the app.

### Overview

The portrait (click to change class, ascendancy or level), name, level and headline DPS; tiles for
EHP, life, energy shield, mana, spirit (PoE2) and resistances; and movable panels Gear, Skills,
Build health, Defences and Passives. Build health lists problems with links to the tab that fixes
them. "Reset layout" restores the panels.

### Tree

- Toolbar: the tree dropdown with New, Copy, Rename and Delete tree; **Compare** to overlay another
  tree (green to add, red to remove); in PoE2, **Allocate into** All, I or II for weapon sets; the
  level box with **auto**; points used; the search box (Enter jumps); **Class**, **Ascendancy**,
  **Fit**, **Undo**, **Redo**; the tree link menu (**Import link** for a pathofexile.com tree URL,
  **Export link**); **Stat differences** (Ctrl+D); **Power** (p) with **Report** (r).
- PoE1 only: **Timeless** opens the timeless jewel search (jewel, conqueror, socket, weighted
  passives, **Search**, then copy or trade the best seeds).
- Mouse: click a node to allocate or remove it; hold Shift to trace a path; right-click an
  attribute node to pick Strength, Dexterity or Intelligence; right-click an allocated mastery to
  change its effect; hover a jewel socket to see its jewel and radius.
- Keywords: underlined words in a node tooltip are the game's keywords. Hold Alt over a node to keep
  its tooltip in place, then hover an underlined word to read the game's definition; definitions
  link further. Click a word to pin its definition; pinned ones stay after Alt is released, drag
  by their title, and close with their X or Esc. Only the current tree version of each game has them.
- Keys: + and - zoom, h centres on the class, a on the ascendancy, f fits, / searches, Esc closes
  pinned keywords, then clears; Ctrl+Z and Ctrl+Y undo and redo on this tab.
- The toolbar can dock at the bottom. In PoE2, Settings, Appearance has "Ascendancy in the tree
  center".

### Skills

The skill set dropdown and its actions, **New socket group** (Ctrl+N), **Copy group**, **Paste
group**, default gem level and quality, a sort stat, and support filters. The left list holds the
socket groups (set the main skill, reorder, right-click or Delete to remove). The right side edits
the selected group: label, Full DPS and Enabled checkboxes, and the gem table. The gem adder
("Add a gem…") has a **DPS** button that scores each support for the selected skill. In PoE1 a row
under the label sets **Socketed in** (the item the group's gems sit in, so they get its "socketed
gems" modifiers), shows that item's sockets with an **Optimise sockets** button that recolours them
to fit the gems, and picks the item's **Imbued support** (one per item; set Socketed in first).

### Items

- Toolbar: item set dropdown, weapon set I or II, **Craft item…**, **New item from text**, **Paste
  item** (Ctrl+V), **Trader…**, **Stat differences** (Ctrl+D).
- Craft item… picks rarity, type and base. Hovering a base in the Base list shows its tooltip and
  what equipping it would change.
- Column 1, Equipment: every slot with a picker; the magnifier is **Buy similar**; right-click a slot
  for Add unique, Add template, Craft item… and Remove from slot. Sub-tabs Squire (PoE2) and
  Wickerman appear only when the build uses them.
- Column 2, Items in build: **Equip**, **Edit**, delete.
- Column 3, the database: Uniques, Templates and saved templates, with search (Ctrl+F) and a type
  filter. **Equip** adds an item and equips it. Double-click a name (or press Enter on it) to open
  the item first: pick a variant, then **Add to build** without equipping, or choose a slot and
  **Add & equip**.
- Clicking an item opens its detail: quality, item level, prefixes and suffixes with tiers and
  rolls, modifiers, sockets, runes, anoints, corruption and more. Its header has **Save as
  template**, **Buy similar**, and a pop-out icon that opens the item in its own window. While that
  window is open, it switches to each item you click, equip or add. An equipped item has **Find
  upgrades on trade…**.

### Calcs

Every calculation PoB makes, in collapsible cards grouped under Offence, Defence and Other, with
search (Ctrl+F) and a mode (Unbuffed, Buffed, In combat, Effective DPS). Hover a value for its
breakdown; click to pin it.

### Config

The config set dropdown, "Filter options…" (Ctrl+F), **Relevant only**, and PoB's options such as
enemy, charges and conditions. **Custom Modifiers** holds mod groups with **Browse…** for a mod
browser and **Add mod group**.

### Notes

Free text for the build, with colour buttons and an **Edit** toggle. Notes save with the build.

### Party

Paste a support build's code or link and **Import** what it gives (auras, curses, warcries, link
skills, party member stats, enemy conditions), with **Clear**, **Disable effects** and **Rebuild
all**.

### Optimize

- **Review**: findings ranked high, medium and low, each with a suggested fix, plus key facts.
- **Gear**: pick Aim (Balanced, Defence, Damage), Item level, Rolls (Perfect, Good, Average) and the
  slots, then **Optimize {n} slots**. Each proposed item has **Apply**, and **Trade**, which opens
  Buy similar for that item so the user can search the trade site for it. **Apply all** applies
  every proposal.
- **Cluster jewels** (PoE1): **Find cluster jewels**, then **Apply**.
- **Tree**: pick a stat, **Scan** for the best nodes to add and the weakest allocated ones, or
  **Plan** a number of points and **Allocate all**.
- Results stay while the user visits other tabs and clear when another build opens.

### Compare

"Compare with" another build (**Add build…** takes a code, link or saved build), then the Summary,
Tree, Items, Skills and Config panes with **Only differences**. The Tree pane can copy their tree
into this build; the Items pane has **Buy similar** and **Use theirs** per slot.

## Settings (Ctrl+,)

- **Appearance**: language, theme (System, Dark, Light, Wraeclast, Abyss, Breach, Ritual,
  Delirium), cursor, contrast, interface scale (Ctrl+= and Ctrl+- change it, Ctrl+0 resets it),
  "Always show tabs as icons", and in PoE2 "Ascendancy in the tree center".
- **Startup**: "Start with an empty build". Off reopens the last build; on starts empty, and the
  Builds tab still offers the last build under Unsaved session.
- **Numbers and defaults**: thousands and decimal separators, default gem quality, default level
  for new builds, default affix roll.
- **Assistant**: choose and set up the provider (Claude Code, Codex, GitHub Copilot, Cursor, Grok,
  Kimi Code, OpenCode, Antigravity, or Ollama on this computer), sign in, pick models, and "Ask
  before each change".
- **MCP server**: "Enable MCP server" and the port, with the address, token and ready-made commands
  for connecting a client.
- **Experimental**: decision models, voice input (speech is turned into text on this computer),
  "Open trade searches in the app", and the Squire panel.
- **Updates**: version, **Check for updates**, then Update and Restart. The app also checks once
  shortly after start.
- **Diagnostics**: the log folder and **Save report…** for bug reports.

## The assistant panel

Opened with the status bar icon, Ctrl+L or Search. It sits on the right; drag its edge to resize,
or **Pop out** to its own window. It runs on the user's own plan with the provider chosen in
Settings, Assistant. The header has the thread list (search, pin, rename, delete) and a new-thread
button. The composer has the model picker, reasoning effort, speed, and **Auto** or **Supervised**
for changes; Supervised asks before each change. Every web lookup asks first. After a reply that
changed the build, a bar shows the stat changes with **Keep** and **Undo**. Enter sends,
Shift+Enter adds a line, "/" lists the tools.

## Trade

- **Trader…** on the Items tab searches upgrades for every slot, weighted by the build. Live prices
  and whispers need **Sign in with Path of Exile** with trade permission; without it the Trader
  still builds the search links.
- **Buy similar** (Items, Compare, item windows, and **Trade** on Optimize proposals) opens a
  dialog with league, listing status and the item's mods to tick, then **Open search**.
- Searches open in the browser, or in an app window when Settings, Experimental, "Open trade
  searches in the app" is on.

## Not in the app

- Dropping a build file onto the window does nothing. Open files with **Open file…**, or open an
  .xml with PoB Redux from the file manager.
- There is no GPU setting. On Linux, `POB_REDUX_GPU=1` turns WebKit's DMA-BUF renderer back on.
