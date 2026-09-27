# Library (Path of Exile 2)

Game knowledge that PoB does not carry, written down so the assistant can give
correct build advice instead of guessing. The `library` tool serves these files
when the open game is PoE2 and the files in [../poe1/](../poe1/README.md) when it
is PoE1.

PoB knows how to calculate a build. It does not know how many passive points a
level 21 character has, which skill gem drops at which level, or that 30 spirit
is not enough for two heralds. That is what these files are for.

> **Early access, with a known expiry date.** Written against **0.5.5**.
> Path of Exile 2 launches **1.0 on 11 December 2026**, which brings **Acts 5 and
> 6** (the interludes are removed), the **Duelist** class, new sword skills and
> endgame additions.
>
> Expect the campaign files to need real rework at launch:
> [00-progression-and-points.md](00-progression-and-points.md) and
> [10-progression-curve.md](10-progression-curve.md) are built on the Acts 1–4
> plus interludes structure and the 24-quest-point total, all of which change.
> The mechanics files should mostly survive; re-check the numbers anyway.

## Files

| File | Covers |
|---|---|
| [00-progression-and-points.md](00-progression-and-points.md) | Levels, the passive point budget, quest rewards, spirit |
| [01-defences.md](01-defences.md) | Armour, evasion, energy shield, block, resistances, the damage calculation order |
| [02-damage.md](02-damage.md) | Damage types, conversion, crit, accuracy, penetration, skill speed |
| [03-ailments.md](03-ailments.md) | Ignite, shock, freeze, chill, bleed, poison, stun, electrocute and the rest |
| [04-skills-and-gems.md](04-skills-and-gems.md) | **Gem tier to character level**, support rules, meta gems, spirit costs |
| [05-sustain-and-utility.md](05-sustain-and-utility.md) | Leech, regeneration, recoup, flasks, charms, curses, marks |
| [06-tree-and-emotions.md](06-tree-and-emotions.md) | Passive tree structure, attribute nodes, distilled emotion anoints |
| [07-advising-builds.md](07-advising-builds.md) | How to turn the above into a build recommendation |
| [08-keywords.md](08-keywords.md) | Short definitions: blind, exposure, withered, impale, rage, pin and the rest |
| [09-buildcraft.md](09-buildcraft.md) | **What 63 published builds actually do** — tree composition, support depth, skill swaps |
| [10-progression-curve.md](10-progression-curve.md) | Stage-by-stage targets: points, skills, supports, gear, uniques |
| [11-gear-and-charms.md](11-gear-and-charms.md) | What goes in each gear slot, and why charms carry the unique budget |
| [12-playstyle-and-buttons.md](12-playstyle-and-buttons.md) | Button count as a design goal, automation via triggers, why boots need movement speed |
| [13-game-constants.md](13-game-constants.md) | **Hard numbers from PoB's engine** — attribute bonuses, every cap, charges, thresholds |
| [14-keystones.md](14-keystones.md) | All 33 keystones with their downsides |
| [15-runes-and-augments.md](15-runes-and-augments.md) | Runes, soul cores, augment sockets |
| [16-evaluating-changes.md](16-evaluating-changes.md) | **Guard rails for judging a change**: requirements are a maximum, flat vs increased vs more, the sheet vs the build, dead stats, a checklist |

## Where this came from

70 Mobalytics guide pages and Maxroll's campaign reference, read and condensed
rather than copied. Each file lists its sources at the bottom. Mobalytics blocks
plain HTTP fetching, so those pages need a real browser to read
(`agent-browser read <url>` works).

Files 09 to 11 come from a different source: **63 published build guides, 324
saved stages**, in `Documents/My Games/Path of Exile 2/BuildPlanner`. Those files
were parsed and counted rather than read, so every number in them is measured.
They describe what experienced players actually ship, which is not always what
the mechanics guides imply.

One table in file 04 is not from any guide either. The **gem tier to character
level** mapping was derived by cross-referencing PoB's `Tier` field against the
`level_interval` values in those same build files. PoB carries no character level
requirement for gems, so without that table there is no way to tell whether a gem
is usable at a given level.

Files 13 to 15 come from two further sources: **poe2db** (`https://poe2db.tw/us/`,
which serves plain HTTP fine, unlike Mobalytics) and **PoB's own data tables**,
read out of the vendored engine.

## When sources disagree

They do, in three places found so far. The rule: **PoB wins for anything the app
reports**, because the app's numbers come from PoB's calculation.

| Topic | Guides say | PoB says | Use |
|---|---|---|---|
| Accuracy per dexterity | 6 (Mobalytics) / 8 (poe2db) | **6** | 6 |
| Bleed magnitude | 20% per second | **15% per second** | 15% |
| Weapon requirement on Giant's Blood | doubled | **tripled attributes** | tripled |

[13-game-constants.md](13-game-constants.md) carries the engine values and is the
tie-breaker.

## Keeping it current

Add to these files as gaps show up in the assistant's answers. A wrong answer
that traces back to missing knowledge is a reason to write a new section here,
not to grow the system prompt.
