# Progression curve

What a PoE1 build looks like at each stage from Act 1 to the endgame. Use the tables to size advice for the
character in front of you: a level 30 character needs the Act 3–5 row, not the endgame build.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame.

## Where the numbers come from

The stage rows are measured from **136 Maxroll guide loadouts** (27 guides) in our PoE1 corpus. Each loadout was
put in a stage by its name ("Act 1", "Early Maps", "Red Maps", "Aspirational") and its points spent. The level is
PoB's own estimate from the points spent, the same estimate PoB's Auto level button uses.

Read two limits before quoting a campaign row:

- **Campaign gear is shared.** 5 of the 7 guides with more than one campaign loadout use one gear set for all of
  them. Tree, skills and links change per stage; gear, flasks and life mostly do not.
- **Every loadout keeps PoB's default resistance penalty of ‑60%.** A campaign loadout shows resistances as they
  would be after Act 10.

Map-stage loadouts are reliable: 110 loadouts use 106 different gear sets.

## Points and levels by act

PoB's act table (`Modules/Build.lua`) estimates the level at each act and the quest points earned before it:

| Entering | PoB level | Quest points so far |
|---|---|---|
| Act 2 | 12 | 2 |
| Act 3 | 22 | 4 |
| Act 5 | 40 | 7 |
| Act 6 | 44 | 9 |
| Act 8 | 54 | 15 |
| Act 10 | 64 | 20 |
| Maps (Act 10 done) | 67 | **23** |

Killing all three bandits in Act 2 adds 1 point in PoB. `build_summary` gives `pointsAvailableMin` and
`pointsAvailableMax` from this table. See [00-progression-and-points.md](00-progression-and-points.md).

PoB also suggests a Labyrinth for each level: Normal from 33, Cruel from 55, Merciless from 68, Uber from 75. PoB
allows 8 ascendancy points.

## The stage table

Medians. "Keys" is `build_summary`'s count of `active` skills; "on" is `persistent`; "trig" is `trigger`.

| Stage | Loadouts (guides) | Points | Level | Main link | Keys / on / trig |
|---|---|---|---|---|---|
| Act 1–2 | 13 (8) | 17 | 16 (10–27) | 3-link in 12 of 13 | 6 / 2 / 0 |
| Act 3–5 | 4 (4) | 51.5 | 44 (37–46) | 4-link in 4 of 4 | 7 / 3 / 0.5 |
| Act 6–10 | 9 (8) | 83 | 64 (54–67) | 4-link in 9 of 9 | 9 / 4 / 1 |
| Entering maps | 6 (6) | 89.5 | 68 (64–75) | 4-link in 3 of 6 | 6 / 2.5 / 2 |
| Early maps | 22 (17) | 112 | 90 (81–95) | 5-link 11, 6-link 11 | 6 / 4 / 3 |
| Mid and red maps | 14 (10) | 119.5 | 97 (92–99) | 6-link in 9 of 14 | 7.5 / 4 / 2.5 |
| Endgame | 52 (25) | 121 | 98 (90–100) | 6-link in 31 of 52 | 6 / 4 / 3 |
| Aspirational | 16 (7) | 123 | 100 (97–100) | 6-link in 13 of 16 | 8.5 / 4 / 3 |

"Main link" counts the main skill and its supports.

## Links on the main skill

The guides step up in three moves:

1. **3-link through Acts 1 and 2** (12 of 13 loadouts).
2. **4-link from Act 3 to the end of the campaign** (13 of 13 loadouts in Acts 3–10).
3. **5- or 6-link once mapping starts.** All 22 early-map loadouts have one: 11 of each.

At the end, **246 of 324** ladder characters with a main link run it as a 6-link. PoB's socket limits decide
where a link can go: 4 sockets on helmet, gloves and boots, 6 on body armour and two-handed weapons, 3 on
one-handed weapons and shields.

## The main skill often changes

8 of the 10 guides with both campaign and map loadouts change the main skill on the way:

| Levelling | Later | Endgame |
|---|---|---|
| Ground Slam | Static Strike | Smite of Divine Judgement |
| Ground Slam | Static Strike | Flicker Strike |
| Ground Slam | Sunder | Ice Crash |
| Spectral Throw | Sunder | Cyclone |
| Rolling Magma | Firestorm | Reap |
| Kinetic Bolt | Kinetic Fusillade, Bane | Essence Drain with Bane of Condemnation |
| Wave of Conviction | | Storm Burst of Repulsion |
| Rolling Magma | Summon Raging Spirit | Raise Spiders |

The Winter Orb and Ice Nova guides keep one skill, but their first loadout is already at level 54 or later. When
asked for a levelling build, give the levelling skill and the level where the swap happens. Check the gem level
with `list_gems` (`req_level`).

## Resistances

PoB applies the campaign penalty to fire, cold, lightning **and chaos** resistance. Set it with `set_config`
(`resistancePenalty`: 0, ‑30 or ‑60; PoB's default is ‑60).

| Point in the campaign | Penalty | Gear and tree must give for 75% |
|---|---|---|
| Before Act 5 is done | 0% | +75% |
| After Act 5 | ‑30% | **+105%** |
| After Act 10 (all maps) | ‑60% | **+135%** |

Helping Alira in Act 2 gives +15% to all elemental resistances in PoB. Maxroll's campaign guide says to cap
elemental resistances at 75% while playing through Act 5, before the Kitava fight applies the first penalty.

Measured at 75% elemental resistances with the ‑60% penalty:

| Stage | Capped |
|---|---|
| Early maps | 17 of 22 guide loadouts |
| Mid and red maps | 14 of 14 |
| Endgame | 44 of 52 |
| Aspirational | 16 of 16 |
| Ladder characters | 266 of 327 |

A character entering maps with uncapped resistances is the most common gap. `sanity_check` reports it under
"resistances".

## Life and energy shield by level

PoB's base life is 38 plus 12 per level, plus 1 per 2 strength. That is 230 at level 16 and 854 at level 68
before the tree and gear.

| PoB level | Guide loadouts | Median life (life builds) | Median ES (ES builds) |
|---|---|---|---|
| 1–20 | 7 | 1,364 | |
| 21–40 | 7 | 1,641 | |
| 41–60 | 7 | 2,273 | |
| 61–75 | 11 | 2,788 | |
| 76–89 | 11 | 3,727 | |
| 90–100 | 93 | **5,032** (64 builds) | 10,492 (29 builds) |
| Ladder, level 100 | 327 | 5,123 (226 builds) | 9,009 (67 CI builds) |

The rows up to level 75 use shared campaign gear, so read them as an upper bound. Map stages have real per-stage
gear: early maps median 4,246 life, mid and red maps 5,292, endgame 4,578.

## Flasks and movement speed

- **Quicksilver Flask** needs level 4 in PoB. Maxroll says the Act 1 and Act 2 quests give one each, and that a
  new character runs two life and two mana flasks.
- Maxroll's quest flasks: Granite, Ruby, Topaz and Sapphire from Act 5, Jade from Act 7, Quartz from Act 10.
- PoB's life flask bases: Divine Life Flask at level 60 (2,400 life), Eternal Life Flask at 65.
- From early maps on, the guides fill all 5 flask slots: 20 of 22 early-map loadouts, and 78 of 82 later ones.
- A Quicksilver is in 21 of 22 early-map loadouts and 238 of 327 ladder characters.

Boots movement speed prefixes in PoB's data need item level 1 (10%), 15 (15%), 30 (20%), 40 (25%), 55 (30%) and
86 (35%). So 25–30% boots are available through the campaign. PoB's movement speed multiplier, median by
stage:

| Stage | Movement speed |
|---|---|
| Act 1–2 | ×1.17 |
| Act 6–10 | ×1.29 |
| Early maps | ×1.83 |
| Endgame | ×1.73 |
| Ladder | ×1.98 |

## When the uniques arrive

Median uniques in the 10 gear slots:

| Stage | Uniques |
|---|---|
| Campaign (26 loadouts) | 0; one unique seen, in 2 loadouts |
| Entering maps | 0.5 |
| Early maps | 1.5 (up to 5) |
| Mid and red maps | 3 |
| Endgame | 3 (up to 9) |
| Aspirational | 4 |
| Ladder | 4 (middle half 2–6) |

Build campaign advice on rares and gems. Uniques come in with mapping.

## Using this when advising

1. **Find the row.** Take the level from `build_summary` and the points from `pointsAvailableMax`.
2. **Match the link to the stage.** A 3-link at level 15 is on target. A 4-link at level 90 is the first thing
   to fix.
3. **Set the right resistance penalty** before judging a campaign character's resistances.
4. **Name the swap.** If the build changes main skill, say which skill and at what level.
5. **Check `sanity_check`** for uncapped resistances, empty flask slots and unspent points.

## Sources

- PoB: `Modules/Build.lua` (act table, level estimate, Labyrinth suggestion), `Modules/CalcSetup.lua` (base life,
  resistance penalty, bandit rewards), `Modules/ConfigOptions.lua` (`resistancePenalty`), `Data/Bases/flask.lua`,
  `Data/ModExplicit.lua` (boots movement speed tiers), `Data/Bases/*.lua` (socket limits).
- Corpus: `corpus/poe1/index.json`, `stages.jsonl` and `xml/`, re-read through PoB 2.67.2 with each loadout
  selected and levels re-estimated from points spent.
- Maxroll: https://maxroll.gg/poe/getting-started/campaign-guide,
  https://maxroll.gg/poe/getting-started/flasks-for-beginners,
  https://maxroll.gg/poe/getting-started/defenses-for-beginners, and the 27 guides under
  https://maxroll.gg/poe/build-guides/ that the corpus loads.
