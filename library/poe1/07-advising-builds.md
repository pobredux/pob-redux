# Advising on builds

How to turn the rest of this library into a build recommendation for Path of
Exile 1. Read this once per conversation, then the topic the question is about.

Written against 3.29 (PoB 2.67.2, tree 3_29).

## Start by pinning the character down

Three facts decide everything else, and PoB only knows them if they are set.

1. **Level.** Call `set_level` if the request names one. Every number PoB reports
   depends on it, and so does which gem levels the character can use.
2. **Passive point budget.** Levels give one point each after level 1, and
   quests and the bandit choice add more as the campaign goes on. `build_summary`
   gives `pointsAvailableMin` and `pointsAvailableMax`; quote the range and say
   what campaign progress it assumes. See
   [00-progression-and-points.md](00-progression-and-points.md).
3. **Campaign stage.** Resistances carry a penalty after Act 5 and Act 10. PoB
   assumes the Act 10 penalty (**-60%**) unless the config says otherwise, so a
   campaign character reads worse on the sheet than in game. Check `get_config`
   before calling resistances too low for an early character.

State these back to the user before recommending anything.

## Then read the build that exists

- `build_summary` for the level, main skill, links, every skill with whether it
  needs a keypress, reservation, flasks, bandit and pantheon.
- `sanity_check` for the problems PoB can already see, ranked.
- `get_items`, `get_skills` and `get_tree_state` for the detail.

Do not recommend from memory of what the build "probably" looks like.

## Picking skills

1. **Call `list_gems`.** Always. Do not name gems from memory; transfigured and
   awakened gems in particular change between patches.
2. **Read `req_level` and `gem_level`.** `req_level` is the character level the
   gem needs at gem level 1; `gem_level` is the highest level the build's
   level allows.
3. **Check attribute requirements** (`req_str`, `req_dex`, `req_int`, then
   `build_summary.requirements` once socketed). The character needs the highest
   single source, never the sum. See
   [16-evaluating-changes.md](16-evaluating-changes.md).
4. **Call `list_valid_supports`** for the supports that can apply, and
   `sort_by_dps` to rank them by PoB's own numbers.
5. **Check the links.** A support only works on a skill in the same link, and the
   item decides how many links there are. See
   [04-skills-and-gems.md](04-skills-and-gems.md).

Aim for one main skill that carries damage, supported toward one scaling type,
plus utility: movement, a curse, auras, a guard skill.

## Picking gear

- **`search_item_db` then `equip_from_item_db`** for uniques. Do not write item
  text by hand; a made-up unique will not calculate correctly.
- **`optimise_gear`** for rares, once for every slot in question. It leaves
  flasks alone.
- **Resistances first.** Capping fire, cold and lightning at 75% after the
  campaign penalty beats any damage upgrade until it is done.
- **Give every slot its job.** See [11-gear-and-flasks.md](11-gear-and-flasks.md):
  the body armour and a two-handed weapon carry the main skill's six links, boots
  carry movement speed, jewellery and belt carry life and resistances.

## Stage the build

Guides stage a build: Act 1, Act 5, Act 10, early maps, red maps, aspirational.
Each stage assumes different gems, links, gear and point counts. When the user
asks about a levelling character, give the stage they asked for, not the endgame
version with a note that it needs level 95. The measured stage targets are in
[10-progression-curve.md](10-progression-curve.md).

## Pathing the tree

- **Work from notables, keystones and masteries**, not small passives.
- `search_tree` finds candidates, `node_info` reads them, `node_path_cost` and
  `path_plan` cost the route, `alloc_path` applies it.
- **A mastery needs an effect.** `node_info` lists the mastery's effects with the
  id `alloc_node` takes as `effect`; an effect another mastery holds is marked
  `takenBy`. See [06-tree-and-masteries.md](06-tree-and-masteries.md).
- `tree_suggest` scores every reachable node for one stat, and the weakest
  allocated ones, from PoB's calculation.
- Stay inside the point budget, and say how many points a suggestion costs.

## Defensive review

Find the missing layer rather than adding more of what is already there. The
layers and their numbers are in [01-defences.md](01-defences.md).

1. Avoidance: evasion, block, spell suppression, movement speed.
2. Mitigation: resistances at 75%, armour, energy shield.
3. Pool: life or energy shield, and what reservation has left.
4. Recovery: leech, regeneration, recoup, life flasks. See
   [05-sustain-and-utility.md](05-sustain-and-utility.md).
5. Flasks and pantheon: the cheapest answers to a specific ailment or damage
   type. See [11-gear-and-flasks.md](11-gear-and-flasks.md) and
   [15-pantheon-and-bandits.md](15-pantheon-and-bandits.md).

## Things that are easy to get wrong

- **Another aura when the main skill cannot be paid for.** Read
  `manaUnreserved` against the main skill's cost first; `sanity_check` flags a
  main skill PoB counts as unusable.
- **Recommending leech as burst protection.** Leech fills the pool over time; it
  does nothing against the hit that kills.
- **Treating increases as multiplicative.** Every "increased" line adds to one
  pool. More and less multipliers are where large gains come from.
- **Suggesting a second curse.** The limit is **1** unless something raises it.
- **Comparing builds by tooltip DPS alone.** It is one skill under the config's
  assumptions; curses, exposure and buffs only count when the config enables
  them.
- **Adding attribute requirements together.** Items and gems are compared, and the
  highest one is the requirement.
- **Treating an item's skill as a choice.** A skill with `grantedBy` came with
  the item; it is not a button unless the build plays it, and it cannot be
  removed without changing the item.
- **Swapping a unique for a rare with better numbers.** Read the unique's text
  first; the line that changes a rule is usually why it is worn.
- **Forgetting the bandit and pantheon.** Both live in the config, and a build
  imported without them misses free power. Set them with `set_config`.

## Saying what you do not know

Numbers move between leagues. When a number in this library drives a
recommendation and it matters, say where it came from. A range with its
assumptions stated is more useful than a confident wrong number.

## What the corpus is

Several chapters quote "our PoE1 corpus": 327 ladder characters from poe.ninja
(league Allflame; the top of each ascendancy: 8 softcore, 4 hardcore and 4 solo
self-found) and 136 loadouts from Maxroll guides, all loaded through the same PoB
the app uses. Read [09-buildcraft.md](09-buildcraft.md) for what those builds do,
and [10-progression-curve.md](10-progression-curve.md) for the stage targets.

## Sources

- PoB PoE1: `Modules/ConfigOptions.lua` (resistance penalty default Act 10,
  -60%), `Modules/CalcSetup.lua` (enemy curse limit 1).
- PoB Redux tools: `src-tauri/src/tools.rs`, `crates/pob-engine/lua/bridge.lua`.
- Our PoE1 corpus: `corpus/poe1/index.json`.
