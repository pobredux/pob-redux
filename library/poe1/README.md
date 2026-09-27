# Library (Path of Exile 1)

Game knowledge that PoB does not carry, for the assistant when the app is in PoE1
mode. The `library` tool serves these files when the open game is PoE1 and the
files in [../poe2/](../poe2/README.md) when it is PoE2.

Written against **3.29** (PoB 2.67.2, tree 3_29), league Allflame.

## Files

| File | Covers |
|---|---|
| [00-progression-and-points.md](00-progression-and-points.md) | Levels, the passive point budget, quest rewards, the bandit choice, ascendancy points |
| [01-defences.md](01-defences.md) | Life, energy shield, armour, evasion, block, spell suppression, resistances, the damage order |
| [02-damage.md](02-damage.md) | Damage types, conversion, crit, accuracy, penetration and exposure, skill speed |
| [03-ailments.md](03-ailments.md) | Ignite, bleed, poison, shock, chill, freeze, scorch, brittle, sap, avoidance |
| [04-skills-and-gems.md](04-skills-and-gems.md) | Sockets and links, gem levels and quality, awakened, transfigured and Vaal gems, reservation |
| [05-sustain-and-utility.md](05-sustain-and-utility.md) | Leech, regeneration, recoup, flasks, curses, marks, guard skills, warcries |
| [06-tree-and-masteries.md](06-tree-and-masteries.md) | Passive tree structure, masteries, cluster and timeless jewels, anointments, bloodlines |
| [07-advising-builds.md](07-advising-builds.md) | How to turn the library into a build recommendation; read first |
| [08-keywords.md](08-keywords.md) | Short definitions of PoE1 keywords |
| [09-buildcraft.md](09-buildcraft.md) | What the builds in our PoE1 corpus do |
| [10-progression-curve.md](10-progression-curve.md) | Stage-by-stage targets from the campaign to the endgame |
| [11-gear-and-flasks.md](11-gear-and-flasks.md) | What goes in each gear slot, sockets and links by slot, influence, flasks |
| [12-playstyle-and-buttons.md](12-playstyle-and-buttons.md) | Button count, automation through triggers and auras, movement speed |
| [13-game-constants.md](13-game-constants.md) | Hard numbers from PoB's engine |
| [14-keystones.md](14-keystones.md) | Every keystone with its downside |
| [15-pantheon-and-bandits.md](15-pantheon-and-bandits.md) | Pantheon gods and their souls, the bandit choice |
| [16-evaluating-changes.md](16-evaluating-changes.md) | Guard rails for judging a change, and a checklist |

## Where this came from

Only these sources, by the project's rule for PoE1:

- **PoB's own PoE1 data and code** in `src-tauri/resources/pob1`, which is what the
  app calculates with.
- **Our PoE1 build corpus** in the private `pobredux/corpus` repository (`poe1/`):
  327 poe.ninja ladder characters from Allflame (the top of each ascendancy: 8
  softcore, 4 hardcore, 4 solo self-found) and 136 loadouts from Maxroll guides,
  loaded through the same PoB. Numbers from it are counted, and quoted as "N of M".
- **poedb.tw, maxroll.gg/poe and poe.ninja**, for mechanics and counts PoB does
  not spell out. poewiki.net and mobalytics.gg are also approved sources, but both
  block automated reads, so no chapter uses them yet. Text is written in our own
  words.

Each file lists its sources at the bottom.

## When sources disagree

PoB wins for anything the app reports, because the app's numbers come from PoB.
Where a site and PoB disagree, the chapter gives both and says which the app uses.

## Keeping it current

Refresh the numbers after a new league: rebuild the corpus with
`bun run refresh --game poe1` in the corpus repository, then re-count. Add to these
files as gaps show up in the assistant's answers.
