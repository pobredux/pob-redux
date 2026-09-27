# Buildcraft patterns

What 63 published build guides (324 saved stages) actually do. Measured, not
guessed — every number here was counted from the `.build` files in
`Documents/My Games/Path of Exile 2/BuildPlanner`.

These are what experienced players ship. They are not proofs of optimality, but
where 63 independent authors agree, that agreement is worth treating as a
default.

## How a passive tree is actually spent

Distinct main-tree allocations across the 63 endgame trees (6,397 nodes,
ascendancy and weapon-set excluded):

| Node type | Share |
|---|---|
| Small passives | 41.7% |
| **Travel / attribute nodes** | **35.3%** |
| Notables | 19.0% |
| Jewel sockets | 4.0% |

**Over a third of every tree is "+5 to any Attribute" travel nodes.** This is the
single most surprising number in the dataset, and it reframes what a passive tree
is for. Roughly one point in three buys no damage and no defence — it buys
attributes to meet gem and gear requirements, and distance to reach the notables
that matter.

A path-planner that optimises purely for damage-per-point will therefore
disagree with every published build. The travel nodes are not waste to be
minimised away; they are how the build meets its requirements.

Watch the naming: `intelligence19`, `dexterity19` and `strength5` are all just
**"+5 to any Attribute"**. The cluster name describes the tree region, not the
attribute granted. Never infer the attribute from the node id.

Per endgame build:

| Measure | Median | Max |
|---|---|---|
| Main-tree points | 103 | 132 |
| Weapon-set allocations | 48 | 64 |
| Ascendancy entries | 9 | 10 |
| Jewel sockets | 4 | 10 |

Median 103 main-tree points is a level ~80 character with all 24 quest points —
the level most guides actually target. The ascendancy count of 9 is the start
node plus 8 points.

The planner does not enforce the point cap. Trees above ~123 main-tree points
exist in the data and are aspirational rather than reachable.

## Supports carry builds, not skills

Distribution of support count across all 2,850 skill placements:

| Supports | Count |
|---|---|
| 0 | 351 |
| 1 | 349 |
| 2 | 536 |
| 3 | 529 |
| 4 | 406 |
| **5** | **678** |

**Five supports is the most common configuration.** The zero- and one-support
entries are persistent buffs and situational utility.

Across the campaign this deepens rather than widens:

| Stage | Skills | Total supports |
|---|---|---|
| Act 1 | 6 | 7 |
| Act 3 | 8 | 16 |
| Interludes | 9 | 20 |
| Endgame | 9 | 34 |
| Late endgame | 11 | 40 |

Skill count barely moves (6 → 11) while supports nearly six-fold. **A build gets
stronger by supporting the skills it has, not by adding more skills.** Advice
that answers "my build is weak" with another skill gem is going against every
guide in the dataset.

The "skills" column above counts socketed gems, which overstates how busy a build
is to play. Fewer than half are keys you press: the median endgame build runs
**5 active skills**, and the count of active skills does not grow across the
whole game. See [12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

Average support tier also climbs, from 1.21 (mostly tier I) at the first stage to
1.57 at the last — players upgrade I → II → III in place.

## The supports everyone takes

Top picks across all 8,028 support placements:

| Uses | Support | Tier | What it is |
|---|---|---|---|
| 389 | Elemental Armament II | 2 | Added elemental damage |
| 356 | Prolonged Duration II | 3 | Skill effect duration |
| 321 | Magnified Area II | 3 | Area of effect |
| 252 | Cooldown Recovery II | 4 | Cooldown recovery rate |
| 198 | Rapid Casting II | 4 | Cast speed |
| 187 | Efficiency II | 4 | Reduced cost |
| 162 | Rapid Attacks II | 4 | Attack speed |
| 139 | Concentrated Area | 1 | Less area, more damage |
| 102 | Prolonged Duration I | 1 | Duration |
| 97 | Elemental Focus | 3 | Elemental damage, no ailments |
| 96 | Multishot II | 2 | Extra projectiles |
| 93 | Blind II | 5 | Blind on hit |
| 89 | Muster | 3 | Minion related |
| 82 | Pinpoint Critical | 2 | Critical scaling |

**Almost none of these are flat "more damage" supports.** The most-taken supports
are duration, area, cooldown recovery, speed and cost efficiency — the throughput
and uptime multipliers. This matches what the mechanics guides say about utility
supports beating raw damage supports when extra area creates extra overlaps.

When recommending supports, reach for uptime and coverage before raw damage
multipliers.

## The levelling skill is usually not the endgame skill

Of 54 guides with more than one saved stage, **32 change their main skill**
between the first and last stage, and only 22 keep it.

The recurring pattern is a cheap, always-available skill carrying the campaign
until the real payoff comes online:

| Levelling skill | Becomes |
|---|---|
| Whirling Slash | Twister (3 separate guides) |
| Explosive Grenade | Cluster Grenade, or Flameblast |
| Contagion | Essence Drain |
| Tempest Flurry | Staggering Palm |
| Wind Blast | Falling Thunder |
| Rolling Slam | Harbinger of Madness |
| Volcano | Entangle |

**Consequence for advice:** when asked for a low-level build, do not simply
down-scale the endgame setup. Give the levelling skill that works now and name
the skill it becomes, with the level it arrives at. "Whirling Slash until 22,
then swap to Twister" is the shape of real guidance.

## Where the passive investment goes

Non-travel clusters, by allocations across the 63 endgame trees:

| Allocations | Cluster |
|---|---|
| 519 | criticals |
| 274 | jewel sockets |
| 222 | energy shield |
| 196 | attack |
| 185 | attack speed |
| 176 | elemental |
| 164 | evasion and energy shield |
| 149 | area attacks |
| 147 | projectiles |
| 104 | slow mitigation |
| 99 | movement speed |

**Criticals is the largest single investment by a wide margin** — around 8 nodes
per build. Critical scaling in PoE 2 rewards this because flat critical chance
raises the base that every multiplier then works on
(see [02-damage.md](02-damage.md)).

**Slow mitigation at 104 allocations** is the defensive stat outsiders never
think of. Being slowed is what gets characters killed, and experienced builds
pay for it.

## Weapon-set points are an endgame tool

Weapon-set allocations are **zero through the entire campaign** in this dataset,
then jump to a median of 48 at endgame. Just over half of all saved stages
(171 of 324) use them at all.

The median of 48 is consistent with 24 points allocated independently in each of
the two sets.

Do not include weapon-set planning in campaign advice. Do raise it for an endgame
build that has not used it — it is free power that half the field leaves on the
table.

## Uniques are an endgame layer

Campaign stages in this dataset use **zero unique items**. Uniques appear only at
maps and after:

| Stage | Median uniques |
|---|---|
| Campaign (all acts) | 0 |
| Early endgame / maps | 3 |
| Endgame | 5 |
| Late endgame | 8 |

**Never build a campaign recommendation around a unique.** Rares and bases carry
the campaign.

## What the archetype spread looks like

Guides per base class, from 63 guides:

| Guides | Class | Most-used ascendancy |
|---|---|---|
| 12 | Mercenary | Gemling Legionnaire (8) |
| 9 | Druid | Oracle (7) |
| 8 | Ranger | Deadeye (6) |
| 7 | Witch | Infernalist (3) |
| 7 | Huntress | Spirit Walker (5) |
| 6 | Warrior | Titan (5) |
| 5 | Monk | Martial Artist (3) |
| 5 | Sorceress | Stormweaver (2) |

Gemling Legionnaire is the most-published ascendancy at 8 guides. Chronomancer,
Amazon, Acolyte of Chayula, Warbringer and Blood Mage have one guide each — thin
coverage, so be more cautious asserting what works for those.

## Checklist: does this build look like a real one?

Compare a build against the dataset shape before calling it finished.

1. **Main skill has 4–5 supports.** Anything less on the damage skill is
   unfinished.
2. **Skill count is 6–11 gems, of which 4–5 are buttons.** More skills is not
   more power, and added power should arrive as automation.
3. **Roughly a third of tree points are travel nodes.** If far fewer, check
   whether attribute requirements are actually met.
4. **Notables are around 19% of points.** Far fewer means the tree is wandering
   without arriving.
5. **Some critical investment**, unless the build has an explicit reason not to.
6. **Charms filled** — see [11-gear-and-charms.md](11-gear-and-charms.md).
7. **Weapon-set points used** if this is an endgame build.
8. **Slow mitigation present** somewhere.
9. **A stated main-skill swap** if the build spans levelling and endgame.

## Sources

63 build guides, 324 `.build` files, analysed 2026-09-03. Node classification
resolved against `TreeData/0_5/tree.json`; gem and unique names resolved against
PoB's `data.gems` and `data.uniques`.
