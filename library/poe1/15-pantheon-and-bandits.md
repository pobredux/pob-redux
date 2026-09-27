# Pantheon and bandits

Two free choices every PoE1 character makes in the campaign: the Act 2 bandit quest and the pantheon gods. Both
live on PoB's Config tab. The app sets them with `set_config`, and PoB adds them to every calculation.

Written against 3.29 (PoB 2.67.2).

## Bandits

The Act 2 quest "Deal with the Bandits" has four outcomes. PoB's numbers come from `Modules/CalcSetup.lua`.

| Choice | `set_config` `bandit` value | Reward |
|---|---|---|
| Kill all three (PoB default) | `None` | **1 passive skill point** |
| Help Oak | `Oak` | **+40 maximum life** |
| Help Kraityn | `Kraityn` | **8% increased movement speed** |
| Help Alira | `Alira` | **+15% to all elemental resistances** |

**Kill all gives 1 point in 3.29.** PoB (`ExtraPoints` +1) and poedb ("1 Passive Skill Points") agree. PoB added
the current rewards in 2.43.0, its 3.25 update ("Add support for new Bandit rewards").

Build files made from the character API, such as poe.ninja exports, store Kill all as `Eramir`. PoB treats every
value other than `Oak`, `Kraityn` and `Alira` as Kill all, and its importer maps `Eramir` to `None`.

### What ladder characters pick

In our PoE1 corpus, 327 ladder characters:

| Choice | Characters | Share |
|---|---|---|
| Kill all | 237 | 72% |
| Alira | 73 | 22% |
| Kraityn | 15 | 5% |
| Oak | 2 | 1% |

Alira is more common in hardcore (23 of 83 characters) than in softcore (32 of 160). Of the 28 Maxroll guide files,
21 kill all and 7 help Alira.

### Advising on the bandit

- Kill all is the default. The point goes wherever the tree needs it.
- Alira is worth it when elemental resistances are short. 15% to all three frees affixes on gear.
- To compare, change `bandit` with `set_config`, then read `get_stats` or `build_summary`. The spare point shows
  in `build_summary` as `extraPoints`.

## Pantheon

The pantheon has 4 major gods and 8 minor gods. A character has one of each active.

- **Unlocking.** Defeating a god in Part 2 (Acts 6 to 10) unlocks its power. Maxroll gives the acts: Brine King
  Act 6, Arakaali Act 7, Lunaris and Solaris Act 8. Minor gods come from side quests: Tukohama, Abberath and
  Ryslatha in Act 6, Gruthkul and Ralakesh in Act 7, Yugul in Act 8, Garukhan and Shakari in Act 9.
- **Changing.** Only in a town or hideout, and free.
- **Upgrading.** Put a Divine Vessel in the map device and run a map with the map boss the pantheon names.
  Killing it turns the vessel into a Captured Soul. Take it to Sin to unlock the upgrade. Each major god has 3
  upgrades; each minor god has 1.
- **Scope.** Maxroll says upgrades apply to every character in the league, while each character still unlocks the
  base power in its own campaign. Maxroll also gives a vendor recipe: 5 flasks and 1 Divine Orb for 5 Divine
  Vessels.

### How PoB counts it

`pantheon.applySoulMod` in `Modules/PantheonTools.lua` applies the base power **and every upgrade**. There is no
per-upgrade setting. A character without upgrades is weaker in game than PoB shows. Say so when a pantheon line
matters to an EHP number.

PoB also skips any line its parser cannot read. Those lines have no effect on the app's numbers. They are marked
"not calculated" below.

### Major gods

| God (`pantheonMajorGod`) | Base power | Upgrades (map boss: effect) |
|---|---|---|
| Soul of the Brine King (`TheBrineKing`) | Cannot be stunned if you were stunned or blocked a stunning hit in the past 2 seconds (**not calculated**) | Nassar, Lion of the Seas: 30% increased stun and block recovery · Captain Tanner Lightfoot: 100% chance to avoid being frozen · Shock and Horror: 50% reduced effect of chill on you |
| Soul of Lunaris (`Lunaris`) | 1% additional physical damage reduction and 1% increased movement speed per nearby enemy, up to 8% each | Captain Clayborne, The Accursed: 10% chance to avoid projectiles · Fragment of Winter: 6% reduced elemental damage taken if hit recently · Glace: avoid projectiles that have chained (**not calculated**) |
| Soul of Solaris (`Solaris`) | 6% additional physical damage reduction while only one enemy is nearby · 20% chance to take 50% less area damage from hits (**not calculated**) | Oak the Mighty: 8% reduced elemental damage taken if not hit recently · Kitava, The Destroyer: no extra damage from critical strikes if you took one recently (**not calculated**) · Forest of Flames: 50% chance to avoid ailments from critical strikes |
| Soul of Arakaali (`Arakaali`) | 10% reduced damage taken from damage over time | Maligaro the Mutilator: 20% increased life and energy shield recovery rate if you stopped taking damage over time recently · Armala, the Widow: debuffs on you expire 20% faster · Drought-Maddened Rhoa: +40% chaos resistance against damage over time |

### Minor gods

| God (`pantheonMinorGod`) | Base power | Upgrade (map boss: effect) |
|---|---|---|
| Soul of Abberath (`Abberath`) | 60% less duration of ignite on you | Mephod, the Earth Scorcher: unaffected by burning ground (**not calculated**), 10% increased movement speed on burning ground |
| Soul of Ralakesh (`Ralakesh`) | 25% reduced physical damage over time taken while moving · no extra damage from moving while bleeding (**not calculated**) | Drek, Apex Hunter: corrupted blood cannot stack past 5 on you (**not calculated**) |
| Soul of Tukohama (`Tukohama`) | 3% additional physical damage reduction per second stationary, up to 9% | Tahsin, Warmaker: regenerate 2% of life per second while stationary |
| Soul of Shakari (`Shakari`) | 50% less duration of poisons on you · cannot be poisoned with 3 or more poisons on you (**not calculated**) | Terror of the Infinite Drifts: 5% reduced chaos damage taken, 25% reduced chaos damage over time taken on caustic ground |
| Soul of Garukhan (`Garukhan`) | 60% reduced effect of shock on you | The Blacksmith: cannot be blinded, cannot be maimed |
| Soul of Ryslatha (`Ryslatha`) | Life flasks gain 3 charges every 3 seconds if no life flask was used recently · 60% increased life recovery from flasks used on low life | Gorulis, Will-Thief: enemies you hit recently have 50% reduced life regeneration |
| Soul of Yugul (`Yugul`) | Prevent +50% of reflected damage (**not calculated**) · 50% chance to reflect hexes (**not calculated**) | Varhesh, Shimmering Aberration: 30% reduced effect of curses on you |
| Soul of Gruthkul (`Gruthkul`) | 1% additional physical damage reduction per hit taken recently, up to 5% | Erebix, Light's Bane: enemies that hit you with an attack recently have 8% reduced attack speed (**not calculated**) |

Maxroll's text for Yugul says "take 50% reduced reflected damage". PoB's 3.29 data says "prevent +50% of
reflected damage". Neither affects the app's numbers, because PoB does not calculate that line.

### What ladder characters pick

In our PoE1 corpus, 327 ladder characters:

| Major god | Characters | | Minor god | Characters |
|---|---|---|---|---|
| Brine King | **152** | | Abberath | **100** |
| Lunaris | 96 | | Ralakesh | 76 |
| Solaris | 39 | | Tukohama | 46 |
| Arakaali | 38 | | Shakari | 28 |
| none | 2 | | Garukhan | 26 |
| | | | Ryslatha | 24 |
| | | | Yugul | 16 |
| | | | Gruthkul | 9 |
| | | | none | 2 |

The most common pairs are Brine King with Abberath (63), Brine King with Ralakesh (35) and Lunaris with Ralakesh
(28). In hardcore, Ralakesh is the top minor god (35 of 83 characters). In softcore and SSF, Abberath leads (58 of
160 and 25 of 84).

### Advising on the pantheon

Match the god to what threatens the character. Every choice is free to change in town.

| Threat | Major god | Minor god |
|---|---|---|
| Freeze, chill, stun | Brine King | |
| Many enemies nearby | Lunaris | |
| One hard enemy | Solaris | |
| Damage over time | Arakaali | |
| Ignite | | Abberath |
| Bleeding, corrupted blood | | Ralakesh |
| Poison, chaos damage | | Shakari |
| Shock | | Garukhan |
| Curses | | Yugul |
| Fighting while standing still | | Tukohama |
| Life flask uptime | | Ryslatha |

A power whose lines PoB does not calculate still works in game. Choose it for the effect, and do not expect the
app's numbers to move.

## In the app

1. Read the current choices with `get_config` (keys `bandit`, `pantheonMajorGod`, `pantheonMinorGod`).
   `list_config_options` lists the allowed values.
2. Change them with `set_config`, for example `{"var": "pantheonMajorGod", "value": "TheBrineKing"}`. Use the
   values in the tables above. `None` means no god, or Kill all for the bandit. A `null` value resets the default.
3. `build_summary` reports the current `bandit` (Kill all, Oak, Kraityn or Alira), `pantheonMajorGod` and
   `pantheonMinorGod`, including a change just made with `set_config`.
4. `sanity_check` flags a character at level 60 or above with neither god chosen (low severity).

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/Data/Pantheons.lua` (every god, soul and line),
  `Modules/PantheonTools.lua` (all souls applied), `Modules/CalcSetup.lua` (bandit rewards, pantheon application),
  `Modules/ConfigOptions.lua` (`bandit`, `pantheonMajorGod`, `pantheonMinorGod` options and tooltips),
  `Modules/Build.lua` (legacy load and save of the choices, quest points), `Classes/ImportTab.lua` (`Eramir` maps to
  `None`), `changelog.txt` (3.25 bandit update). Which lines PoB calculates was checked by running each line through
  `modLib.parseMod` with `pobctl eval`.
- PoB Redux: `crates/pob-engine/lua/bridge.lua` (`set_config`, `get_config`, `build_summary`, `sanity_check`).
- Corpus: `S:\_projects_\_poe2_\corpus\poe1\xml\*.xml` (`bandit`, `pantheonMajorGod`, `pantheonMinorGod` on the
  `Build` element) for the 327 ladder characters in `index.json` without the `no-dps` flag, and the 28 guide files.
- poedb: https://poedb.tw/us/Deal_with_the_Bandits (1 passive point), https://poedb.tw/us/Pantheon (unlocking in
  Part 2, changing in town, Divine Vessel upgrades).
- Maxroll: https://maxroll.gg/poe/resources/pantheon (acts per god, Captured Soul and Sin, league-wide upgrades,
  vendor recipe).
