# Progression and points

How many passive and ascendancy points a Path of Exile 1 character has at a given point in the campaign, and
what the campaign takes away. PoB only knows how many points a tree spends. The budget below comes from PoB's
own act table (`Modules/Build.lua`), which the bridge copies as `POE1_ACTS`.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame.

## Levels

Characters run from level **1 to 100**. Every level after the first gives **1 passive point**, so levels alone
give **99**. Base life and mana also grow with level; the formulas are in
[13-game-constants.md](13-game-constants.md).

In our PoE1 corpus, 323 of 327 ladder characters are level 100. A campaign character has far fewer points, so
work out its budget first.

Call `set_level` before anything else when the user names a level. On a levelling build, follow it with
`set_gem_levels`, which lowers every gem the level cannot use yet.

## Passive points from quests

PoB counts **23 quest points** over the ten acts. Each row is the total PoB assumes once an act is done, and the
level it expects the character to have by then.

| Act | Quests that give points (PoB's list) | Points | Running total | Level PoB expects after it |
|---|---|---|---|---|
| 1 | The Dweller of the Deep, The Marooned Mariner | 2 | 2 | 12 |
| 2 | The Way Forward, Through Sacred Ground | 2 | 4 | 22 |
| 3 | Victario's Secrets, Piety's Pets | 2 | 6 | 32 |
| 4 | An Indomitable Spirit | 1 | 7 | 40 |
| 5 | In Service to Science, Kitava's Torments | 2 | 9 | 44 |
| 6 | The Father of War, The Puppet Mistress, The Cloven One | 3 | 12 | 50 |
| 7 | The Master of a Million Faces, Queen of Despair, Kishara's Star | 3 | 15 | 54 |
| 8 | Love is Dead, Reflection of Terror, The Gemling Legion | 3 | 18 | 60 |
| 9 | Queen of the Sands, The Ruler of Highgate | 2 | 20 | 64 |
| 10 | Vilenta's Vengeance (1), An End to Hunger (2) | 3 | **23** | 67 |

PoB's comment marks Through Sacred Ground as the 3.25 Fellshrine reward.

## The bandit choice

The Act 2 bandit quest gives one reward. `set_config` var `bandit` sets it; PoB's default is kill all.

| Choice | `bandit` value | Reward in PoB |
|---|---|---|
| Kill all three | `None` | **+1 passive point** |
| Help Alira | `Alira` | +15% to all elemental resistances |
| Help Kraityn | `Kraityn` | 8% increased movement speed |
| Help Oak | `Oak` | +40 maximum life |

poedb agrees that killing all three gives 1 passive point. A character imported with the bandit "Eramir" is the
kill-all choice; PoB maps it to `None`. Maxroll's campaign guide says the choice can be changed later through a
vendor recipe.

In our PoE1 corpus, 237 of 327 ladder characters killed all three, 73 helped Alira, 15 helped Kraityn and 2 helped
Oak. The pantheon and the bandit trade-offs are in [15-pantheon-and-bandits.md](15-pantheon-and-bandits.md).

## The total

| Source | Points |
|---|---|
| Levels 2 to 100 | 99 |
| Quests | 23 |
| Bandits, kill all | 1 |
| **Total** | **123** |

Maxroll's passive tree guide also gives 123. The tree data carries the same figure (`points.totalPoints = 123`).

Some ascendancy nodes add passive points on top. They read "Grants N Passive Skill Point(s)":

- **Ascendant** (Scion): each "Path of the <class>" node grants **2** and lets the tree start from that class's
  start. Its "Passive Point" nodes grant **1** each.
- **Reliquarian** (Scion): its "Passive Point" nodes grant **1** each.

PoB counts these, and the bandit point, as `ExtraPoints`. One level 100 Ascendant in our corpus spends 128 points.

## How build_summary reports the budget

PoB cannot know how far through the campaign a character is, so the bridge brackets it by level with
`POE1_ACTS`:

- `pointsFromLevels` (in `get_tree_state`) is `level - 1`.
- `questPointsMin` is the total for the last act the level has passed. `questPointsMax` is the next act's total.
- `extraPoints` is PoB's `ExtraPoints`: the bandit point plus any ascendancy nodes that grant points.
- **`pointsAvailableMin` = level − 1 + questPointsMin + extraPoints**, and `pointsAvailableMax` uses
  questPointsMax.

What a fresh build with the default bandit choice reports:

| Level | `pointsAvailableMin` | `pointsAvailableMax` |
|---|---|---|
| 1 | 1 | 3 |
| 12 | 14 | 16 |
| 21 | 23 | 25 |
| 40 | 47 | 49 |
| 50 | 62 | 65 |
| 67 and up | level + 23 | same |
| 100 | 123 | 123 |

Two things to say when quoting it:

1. **It is a range.** Quote both ends and the campaign progress they assume.
2. **The bandit point counts from Act 2 on.** PoB's act table only adds the kill-all point once Act 2 is done,
   and `build_summary` follows it: a kill-all character's range is 0 to 2 at level 1 and 26 to 28 at level 22.

`sanity_check` compares `passivePointsSpent` with the range. It reports "high" when the tree spends more than
`pointsAvailableMax` + 1, and "low" when it spends fewer than `pointsAvailableMin`. An unset level is the usual
cause of a "high" finding.

## Ascendancy points

The Labyrinth gives **2 ascendancy points per difficulty, 8 in total**. PoB's point display suggests which
Labyrinth fits a level, and Maxroll's Labyrinth guide gives the area levels.

| Labyrinth | Unlocked by | Area level | Points | PoB suggests it at level |
|---|---|---|---|---|
| Normal | Trials of Ascendancy in Acts 1 to 3 | 33 | 2 | 33 to 54 |
| Cruel | Trials in Acts 6 and 7 | 55 | 2 | 55 to 67 |
| Merciless | Trials in Acts 8 to 10 | 68 | 2 | 68 to 74 |
| Eternal (Uber) | An Offering to the Goddess, after the campaign | 75 (83 with an enhanced offering) | 2 | 75 to 89 |

- The character picks its ascendancy class after the first Normal Labyrinth.
- The Divine Font can change the ascendancy once all ascendancy points are refunded (Maxroll).
- PoB warns when a tree spends more than 8. `build_summary.ascendancyPointsUsed` counts bloodline nodes as well,
  because bloodlines share the same 8 points. See [06-tree-and-masteries.md](06-tree-and-masteries.md).
- `sanity_check` flags a character of level 20 or more with no ascendancy, and one with fewer than 8 points spent.
  Fewer than 8 is right for a campaign character; say which Labyrinths it has done.

In our PoE1 corpus, 326 of 327 ladder characters spend all 8.

### Classes and ascendancies

`list_classes` gives these ids for `select_class`.

| Class id | Class | Ascendancies (ids 1, 2, 3) |
|---|---|---|
| 0 | Scion | Ascendant, Reliquarian, Luminary |
| 1 | Marauder | Juggernaut, Berserker, Chieftain |
| 2 | Ranger | Warden, Deadeye, Pathfinder |
| 3 | Witch | Occultist, Elementalist, Necromancer |
| 4 | Duelist | Slayer, Gladiator, Champion |
| 5 | Templar | Inquisitor, Hierophant, Guardian |
| 6 | Shadow | Assassin, Trickster, Saboteur |

The tree data still calls the Warden "Raider" internally.

## The campaign resistance penalty

Kitava lowers every resistance twice: once at the end of Act 5 and once at the end of Act 10. The penalty covers
fire, cold, lightning **and chaos**.

| Campaign progress | Penalty | `set_config` `resistancePenalty` |
|---|---|---|
| Before the Act 5 Kitava fight | 0% | `0` |
| After Act 5 | **−30%** | `-30` |
| After Act 10 (PoB default) | **−60%** | `-60` |

Maxroll gives the same values: −30% after each Kitava fight, −60% in total. PoB applies −60% unless the config
says otherwise, so a campaign character reads 30 to 60 points short on every resistance. Set the penalty for the
act the character has reached before judging its resistances. The other resistance numbers are in
[13-game-constants.md](13-game-constants.md).

## Using this when advising

1. **Set the level** with `set_level`, then `set_gem_levels` on a levelling build.
2. **Read the budget** from `build_summary`. Quote `pointsAvailableMin` to `pointsAvailableMax` and say what
   campaign progress it assumes. Take 1 off for a character that has not done the bandit quest.
3. **Set `resistancePenalty`** for a campaign character.
4. **Check `bandit`** in `build_summary`. An imported build may carry the wrong choice.
5. **Match ascendancy points to the Labyrinths done**: 2, 4, 6 or 8.

## Sources

- PoB PoE1 (`src-tauri/resources/pob1`): `Modules/Build.lua` (act table, `EstimatePlayerProgress`, Labyrinth
  suggestion, point warnings), `Modules/CalcSetup.lua` (bandit mods, resistance penalty),
  `Modules/ConfigOptions.lua` (`bandit`, `resistancePenalty`), `Classes/ImportTab.lua` (Eramir mapped to kill
  all), `Classes/PassiveSpec.lua` (`CountAllocNodes`), `TreeData/3_29/tree.lua` (`points`, classes, ascendancy
  nodes that grant passive points).
- PoB Redux: `crates/pob-engine/lua/bridge.lua` (`POE1_ACTS`, `build_summary`, `get_tree_state`,
  `sanity_check`, `list_classes`); budget table produced with `pobctl eval` on a fresh PoE1 build.
- Our PoE1 corpus: `corpus/poe1/index.json`, `stages.jsonl` and `xml/*.xml` (levels, bandit choices,
  ascendancy points on 327 ladder characters).
- poedb: <https://poedb.tw/us/Deal_with_the_Bandits>
- Maxroll: <https://maxroll.gg/poe/getting-started/passive-skill-tree-for-beginners>,
  <https://maxroll.gg/poe/getting-started/labyrinth-guide>, <https://maxroll.gg/poe/getting-started/campaign-guide>,
  <https://maxroll.gg/poe/getting-started/defenses-for-beginners>
