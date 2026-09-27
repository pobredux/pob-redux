# Advising on builds

How to turn the rest of this library into a build recommendation. Written after
watching the assistant give incoherent advice, so most sections here exist
because something went wrong.

## Start by pinning the character down

Three facts decide everything else, and two of them are not in PoB by default.

1. **Level.** Call `set_level`. If the request names a level — "a build for an up
   to level 21 Ranger" — set it before doing anything else. Every number PoB
   reports depends on it, and a build left at level 1 makes the whole answer
   wrong.
2. **Passive point budget.** `(level - 1) + quest points`. Quest points depend on
   campaign progress, not level, so quote a range and say what it assumes. See
   [00-progression-and-points.md](00-progression-and-points.md).
3. **Spirit.** 0 before Act 1, then 30 / 60 / 100. Check it before suggesting
   anything that reserves.

State all three back to the user before recommending anything. If the user asked
for level 21, say "20 points from levels, 24–28 total depending on quests, and
30 spirit if Act 1 is done" — then build inside those limits.

## Then read the build that exists

- `get_character` — class, ascendancy, level.
- `get_sidebar` — the headline numbers.
- `get_tree_state` — what is allocated and what is left.
- `get_items` / `get_skills` — the current setup.
- `sanity_check` — problems PoB already knows about.

Do not recommend from memory of what the build "probably" looks like.

## Picking skills

The failure mode to avoid: four unrelated skills with scattered supports. That
happened because gems were picked by name recognition rather than by looking them
up.

1. **Call `list_gems`.** Always. Do not name gems from memory.
2. **Filter by tier against the character's level**, using the table in
   [04-skills-and-gems.md](04-skills-and-gems.md). Tier 1→level 1, 3→6, 4→10,
   5→14, 7→22, 8→26, 9→31, 11→41, 13→52, 14→58.
3. **Check attribute requirements** (`req_str`, `req_dex`, `req_int` from
   `list_gems`, then `build_summary.requirements` once socketed). The
   character needs the highest single source, never the sum; supports have no
   requirement of their own. See
   [16-evaluating-changes.md](16-evaluating-changes.md).
4. **Call `list_valid_supports`** for the real legal supports on a skill.
5. **Check weapon requirements** — an attack gem needs the right weapon equipped.

Aim for one main skill that carries damage, supported coherently toward a single
scaling axis, plus utility. Not four half-supported skills.

## Picking gear

- **`search_item_db` then `equip_from_item_db`** for real items. Do not invent
  item text — a made-up unique will not calculate correctly.
- **Resistances first.** Characters start at ‑50% to each element. Getting fire,
  cold and lightning to 75% beats any damage upgrade at low and mid level.
- **Match the defence to the attribute.** Strength gear carries armour,
  dexterity carries evasion, intelligence carries energy shield.
- **Watch local vs global** on defence modifiers — see
  [01-defences.md](01-defences.md).
- **Runic ward is free upside on items of level 55 or below.**

## Stage the build

Real build guides are staged, and the planner files show it: separate saved
builds for "Act 1-3", "~Lv40-44", "~Lv60-65", "Endgame", "Uber Endgame". Each
stage assumes different gems, gear and point counts.

When asked for a levelling build, give **the stage that was asked for**, not the
endgame version with a note that it needs level 90. A level 21 character wants
tier 1–5 gems, cheap gear and resistances — not Tornado Shot.

Useful stage boundaries, which line up with the gem tier ladder:

| Stage | Level | Gems available up to |
|---|---|---|
| Early campaign | 1–13 | Tier 4 |
| Mid campaign | 14–25 | Tier 5 |
| Late campaign | 26–40 | Tier 9 |
| Early endgame | 41–60 | Tier 14 |
| Endgame | 60+ | Everything, plus lineage supports |

## Pathing the tree

- **Work from notables and keystones**, not small passives.
- Use `search_tree` to find candidates, `node_info` to read them,
  `node_path_cost` and `path_plan` to cost the route, `alloc_path` to apply it.
- **When the request names an attribute** — "path to Heartstopper choosing int
  nodes on the way" — call `set_attribute_choice` on the travel nodes.
  Allocating them leaves the choice unset, and the default is not what was asked
  for.
- **Check `already_granted`** before pathing to a notable; an amulet instil may
  already provide it.
- Stay inside the point budget, and say how many points the suggestion costs.

## Defensive review

Run down the layers in [01-defences.md](01-defences.md) and find the **missing**
one rather than adding more of what is already there:

1. Avoidance — evasion, block, movement speed.
2. Mitigation — resistances at 75%, armour or energy shield.
3. Pool — life, energy shield, runic ward.
4. Recovery — the right mechanic for the problem, per
   [05-sustain-and-utility.md](05-sustain-and-utility.md).
5. Charms — three slots, and usually the answer when a specific ailment is
   killing someone.

## Things that are easy to get wrong

- **Two heralds on 30 spirit.** Does not work. Check spirit first.
- **Recommending leech as burst protection.** Leech is removed at full resource;
  it does nothing against the hit that kills you.
- **Stacking penetration alongside curses and exposure.** Penetration cannot go
  below 0% resistance, so the two compete.
- **Treating increases as multiplicative.** "Increased projectile damage" and
  "increased lightning damage" are one additive bucket. More/less multipliers are
  where the real gains are.
- **Forgetting the accuracy distance penalty.** Ranged attack builds take up to
  90% less accuracy at 9 metres and beyond.
- **Ignoring the 24 weapon set passive points.** They are a separate pool.
- **Suggesting electrocute without an enabler.** It cannot be inflicted by
  default.
- **Suggesting two curses.** The limit is one; marks are separate and do not
  count.
- **Comparing builds by tooltip DPS alone.** Curses, exposure and penetration do
  not appear in it.
- **Adding attribute requirements together.** Items, gems and the support-gem
  source are compared, and the highest one is the requirement.
- **Treating a weapon-granted skill as a choice.** A group with `grantedBy` came
  with the item; it is not a button unless the build plays it, and it cannot be
  removed or replaced.
- **Swapping a unique for a rare with better numbers.** Read the unique's text
  first; the line that changes a rule is usually why it is worn.

## Saying what you do not know

The game is in early access. Quest rewards, gem tiers and numbers will move at
1.0. When a number here drives a recommendation and it matters, say where it came
from and that it may have changed. A range with its assumptions stated is more
useful than a confident wrong number.

## Reference builds

The user's planner files at
`Documents/My Games/Path of Exile 2/BuildPlanner` are worked examples of what
good advice looks like: staged, coherent, with gems tagged by the level they come
online. Reading one before recommending a similar archetype is worth more than
guessing.

Their JSON carries `level_interval` on every skill and gear piece — the level a
gem or item enters the build. That is where the tier-to-level table in
[04-skills-and-gems.md](04-skills-and-gems.md) came from.

All 63 of them have been counted, and the results are in three files that should
be read alongside this one:

- [09-buildcraft.md](09-buildcraft.md) — what a real tree and skill setup looks
  like, including the finding that a third of every tree is travel nodes and that
  supports rather than skills carry a build.
- [10-progression-curve.md](10-progression-curve.md) — the expected points,
  skills, supports and gear at each stage. Check any recommendation against the
  row for the character's level.
- [11-gear-and-charms.md](11-gear-and-charms.md) — what belongs in each slot, and
  why charms are the highest-value slots in the game.
