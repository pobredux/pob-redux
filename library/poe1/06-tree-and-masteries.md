# Tree and masteries

The PoE1 passive tree, masteries, cluster and timeless jewels, anointments and bloodlines. Counts come from PoB's
3_29 tree data; "ladder characters" are the 327 in our PoE1 corpus.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame.

## Shape of the tree

| Node type | On the main tree |
|---|---|
| Class starts | **7** (one per class) |
| Small passives | 1,476 |
| Notables | **454** |
| Keystones | **48** |
| Masteries | **315**, of 61 kinds |
| Jewel sockets | **21**: 15 regular, 6 large outer sockets |

The tree data also holds nodes nobody can path to: 300 cluster jewel notables, 8 cluster jewel keystones, and 30
nodes that only an anointment grants.

- **Attribute nodes are fixed.** A small attribute node gives a set amount, usually +10 to one attribute. PoE1 has
  no choosable attribute nodes, so the app offers no `set_attribute_choice` in PoE1.
- **The budget** is 123 passive points and 8 ascendancy points. See
  [00-progression-and-points.md](00-progression-and-points.md).
- **Keystones** carry the largest trade-offs. Each one is in [14-keystones.md](14-keystones.md).

In our PoE1 corpus, ladder characters allocate a median of **2 keystones** (40 of 327 take none) and a median of
**5 of the 21 jewel sockets**.

| Most taken keystones | Ladder characters | Most taken notables | Ladder characters |
|---|---|---|---|
| Chaos Inoculation | 67 | Purity of Flesh | 106 |
| Ghost Dance | 48 | Barbarism | 97 |
| Unwavering Stance | 45 | Devotion | 96 |
| Mind Over Matter | 42 | Written in Blood | 93 |
| Versatile Combatant | 41 | Golem's Blood | 92 |
| Zealot's Oath | 36 | Cruel Preparation | 92 |
| Pain Attunement | 35 | Bloodless | 89 |

All seven of those notables give increased maximum life.

## Masteries

- A mastery sits in the middle of a cluster. In PoB's data it links **only to that cluster's notables**, so it
  can be taken once one of them is allocated. It costs 1 point.
- The player picks **one effect** per mastery. **Each effect can be taken only once on the whole tree** (Maxroll).
  Masteries of one kind share an effect list; the tree has 14 Life Masteries, for example.
- 353 distinct effects exist.

In the app:

1. `node_info` on a mastery lists `masteryEffects`, each with the `effect` id and `takenBy` when another mastery
   already holds it.
2. `alloc_node` with `effect` allocates the mastery and its path. On an allocated mastery it changes the effect.

In our PoE1 corpus, ladder characters take a median of **8 masteries** (from 2 to 16).

| Most taken mastery effects | Ladder characters |
|---|---|
| Life: 15% increased maximum life if there are no life modifiers on the body armour | 146 |
| Mana: 12% increased mana reservation efficiency of skills | 139 |
| Block: +1% chance to block spell damage per 5% chance to block attack damage | 96 |
| Leech: 5% of leech is instant | 76 |
| Life: +30 to maximum life | 70 |
| Critical: +25% critical strike multiplier against unique enemies | 68 |
| Shield: +1% chance to block attack damage per 5% block chance on the shield | 63 |
| Life: 10% more maximum life with at least 6 Life Masteries allocated | 61 |

Life Masteries lead by kind: 412 allocations across the 327 characters.

## Cluster jewels

Cluster jewels go only in the **6 large sockets on the outer edge of the tree**, and in the sockets that other
cluster jewels add. Sizes, from Maxroll and PoB's `ClusterJewels.lua`:

| Size | Passives added | Notables | Jewel sockets it adds | Fits in |
|---|---|---|---|---|
| Large | 8 to 12 | up to 3 | 2 | Outer large sockets |
| Medium | 4 to 6 | up to 2 | 1 | Outer large sockets, large cluster sockets |
| Small | 2 to 3 | up to 1 | 0 | Any of the above, and medium cluster sockets |

- The enchantment "Added Small Passive Skills grant: ..." sets what every small passive on the jewel gives.
- Each notable comes from a "1 Added Passive Skill is <notable>" modifier. PoB knows 300 cluster notables.
- PoB lists **8 cluster keystones**. Seven come from unique Small Cluster Jewels, for example Kitava's Teachings
  (Disciple of Kitava), Calamitous Visions (Lone Messenger) and One With Nothing (Hollow Palm Technique).
- Some affixes roll only on cluster jewels of item level 50, 68, 75 or 84 and above (Maxroll).

In our PoE1 corpus, 148 of 327 ladder characters use cluster jewels. Between them they socket 241 large, 164
medium and 101 small rare or magic jewels, plus 35 Voices and 14 Megalomaniac.

## Timeless jewels

A timeless jewel rewrites the passives in its **large radius**. Only one can be equipped. PoB supports six seed
types:

| Jewel | Legion | Seed range |
|---|---|---|
| Glorious Vanity | Vaal | 100 to 8,000 |
| Lethal Pride | Karui | 10,000 to 18,000 |
| Brutal Restraint | Maraketh | 500 to 8,000 |
| Militant Faith | Templar | 2,000 to 10,000 |
| Elegant Hubris | Eternal | 2,000 to 160,000, in steps of 20 |
| Heroic Tragedy | Kalguur | 100 to 8,000 |

- Each jewel also names a conqueror, and the conqueror decides which keystone the jewel adds.
- 3.29 adds five **Abyss** timeless jewels (Tecrod, Ulaman, Kurgal, Amanamu, Zorath). They change nodes from
  components stored on the jewel, not from a seed.
- **The assistant cannot search seeds.** The app can: the **Timeless** button in the tree view runs PoB's seed
  search. Send the user there.
- `suggest_unique_jewels` does not score timeless jewels. It lists them under `notScored`, because PoB needs the
  exact seed and socket.

In our PoE1 corpus, 90 of 327 ladder characters use one: Lethal Pride 32, Elegant Hubris 25, Militant Faith 15,
Glorious Vanity 10, Brutal Restraint 8.

## Anointments

An amulet anointed with **three oils** allocates one notable without a passive point.

- PoB knows **472 anointable nodes**: 442 tree notables, plus 30 that exist only as anointments (29 notables and
  the keystone Worship the Blightheart). Every one of those 30 needs a Prismatic Oil.
- PoB's recipes use 14 kinds of oil.
- **Talisman amulets cannot be anointed** in 3.29. Their implicit is now an enchantment, and anointments are
  enchantments too (Maxroll); PoB blocks it.
- Anointing a notable that is already allocated gives nothing.
- PoB counts an anointed notable as allocated. `path_plan` returns `already_granted` for it, so do not path to it
  or count it against the budget.

In our PoE1 corpus, 286 of 327 ladder amulets are anointed. The most common are As The Mountain (14), Force of
Darkness (13), Sovereignty (11), Disciple of the Slaughter (11), Whispers of Doom (10) and Inveterate (10). 26
characters anoint an anoint-only notable, mostly Force of Darkness.

## Bloodlines

A bloodline is a second, small ascendancy tree. It pairs with any ascendancy and **shares the same 8 ascendancy
points** (poedb). Each is unlocked by defeating a particular endgame boss; poedb gives The Trialmaster for the
Chaos Bloodline, and It That Was Tul with It That Was Esh for the Breachlord Bloodline.

- `list_classes` returns them as `secondaryAscendancies`. `select_class` takes the id as
  `secondary_ascend_class_id`; 0 removes it.
- PoB counts bloodline nodes in `ascendancyPointsUsed` and warns above 8. `get_tree_state` also reports
  `secondaryAscendancyPointsUsed`.
- PoB hides three older alternate ascendancies from the tree: Warden of the Maji, Warlock of the Mists and
  Wildwood Primalist.

| Id | Bloodline | Notables | Ladder characters |
|---|---|---|---|
| 4 | Chaos | Corruption's Embrace, Deceitful Servant | 7 |
| 5 | Oshabi | The Primal Owl, The Vivid Cat, The Wild Bear | 4 |
| 6 | Nameless | The Unseen Hand (+1 ring slot), Unlight Silhouette | 4 |
| 7 | Catarina | Death Offering, Prolonged Servitude, Umbral Army | 8 |
| 8 | Aul | Precursor's Release, Legacy of the King | 5 |
| 9 | Lycia | Bitter Heresy, Farewell to Flesh, Sinner Saint | 21 |
| 10 | Olroth | Enhanced Starlight, Runic Boon, Volatile Runes | 1 |
| 11 | Farrul | Farrul's Will, Huntleader, Primal Roar | 22 |
| 12 | Delirious | It wasn't me!, That didn't happen!, You're the crazy one! | 9 |
| 13 | Breachlord | Cryogenesis, Esh of the Storm, Tul of the Blizzard | 2 |
| 14 | Saresh | Afarud Ritual, Bitter Lash, Bound Flesh | 0 |
| 15 | Velka | Brine Rot, Salt and Scale, Tide Caller | 4 |
| 16 | Abyssal | Bone Feeders, Conjoined Spire, Gaze of the Lich | 1 |

In our PoE1 corpus, 88 of 327 ladder characters spend points in a bloodline. The usual split is **6 ascendancy
points and 2 bloodline points** (62 characters). Farrul's Huntleader (18) is the most taken bloodline notable.

## Advising on the tree

1. **Work from keystones, notables and masteries.** `search_tree` finds them, `node_info` reads them, `path_plan`
   costs the route, and `tree_suggest` scores reachable nodes from PoB's numbers.
2. **Give every mastery an effect** that no other mastery holds.
3. **Check the amulet** before pathing to a notable.
4. **Send timeless seed searches** to the app's Timeless button.
5. **Take bloodline points out of the 8.** Two bloodline points cost two ascendancy nodes.

## Sources

- PoB PoE1 (`src-tauri/resources/pob1`): `TreeData/3_29/tree.lua` (nodes, links, masteries, recipes, alternate
  ascendancies, points), `Classes/PassiveTree.lua` (hidden legacy alternate ascendancies),
  `Classes/PassiveSpec.lua` (`CountAllocNodes`, mastery selections), `Data/ClusterJewels.lua`,
  `Data/ModJewelCluster.lua`, `Data/Uniques/jewel.lua` (cluster keystones, timeless jewels), `Modules/Data.lua`
  (timeless jewel types and seed ranges), `Classes/ItemsTab.lua` and `Classes/NotableDBControl.lua`
  (anointing), `Modules/CalcSetup.lua` (granted passives), `Modules/Build.lua` (ascendancy point warnings),
  `changelog.txt` (Abyss timeless jewels).
- PoB Redux: `crates/pob-engine/lua/bridge.lua` (`select_class`, `list_classes`, `node_info` mastery effects,
  `path_plan`, `suggest_unique_jewels`, timeless search), `src-tauri/src/tools.rs`,
  `src/lib/views/TreeView.svelte` (Timeless button).
- Our PoE1 corpus: `corpus/poe1/stages.jsonl`, `tree-nodes.json`, `index.json` and `xml/*.xml` (keystones,
  notables, masteries, sockets, jewels, anointments and bloodlines on 327 ladder characters).
- Maxroll: <https://maxroll.gg/poe/getting-started/passive-skill-tree-for-beginners>,
  <https://maxroll.gg/poe/resources/cluster-jewels-explained>,
  <https://maxroll.gg/poe/news/poe-3-29-curse-of-the-allflame-league-branch-update>,
  <https://maxroll.gg/poe/news/3-29-curse-of-the-allflame-reveal-summary>
- poedb: <https://poedb.tw/us/Bloodline_Ascendancy_class>
