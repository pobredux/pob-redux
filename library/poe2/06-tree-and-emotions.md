# The passive tree, and instilled notables

Written against 0.5.5.

## Shape of the tree

- Over 1000 nodes, **6 starting locations**, one per attribute combination.
- **12 base classes** in 0.5.5, two per combination: pure strength, dexterity,
  intelligence, and the hybrids strength/dexterity, intelligence/dexterity,
  strength/intelligence. The **Duelist** arrives at 1.0 on 11 December 2026,
  along with new sword skills — re-check the count and start positions then.
- **Nothing is gated by class.** A Witch can path into the strength section.
  Start position is the only class restriction, and pathing cost is the only
  penalty.

Rough layout:

| Section | Position | Themes |
|---|---|---|
| Intelligence | Top | Spell damage, summoning, elemental, energy shield |
| Dexterity | Bottom right | Ranged, evasion, speed |
| Strength | Bottom left | Melee, armour, defence |

Hybrid classes start between the pure sections.

## Node types

**Small passives** — minor bonuses. Attack speed, spell damage, attributes.

**Notables** — larger bonuses, thematically matching the small passives leading
to them. They are the right unit to reason about: pick the notable you want, then
work out the path. Reasoning node by node over 1000 small passives is the wrong
approach and produces incoherent builds.

**Travel / attribute nodes** — grant **+5 to an attribute of your choice** and
exist to reach distant parts of the tree. Three things matter here:

- They are how builds meet gem and equipment attribute requirements. **Levelling
  grants no attributes at all** (see [13-game-constants.md](13-game-constants.md)),
  so the tree and gear are the only sources.
- **They are not dead points.** Each attribute carries an inherent bonus: +2 life
  per strength, +6 accuracy per dexterity, +2 mana per intelligence. Twenty
  travel nodes into strength is +200 maximum life. This is why they make up 35%
  of published trees.
- **The attribute is chosen per node and can be respecced.** When someone asks to
  path somewhere "taking int nodes", these are the nodes they mean, and the
  choice must actually be set — allocating the node is not enough.

**Keystones** — build-defining, with a large upside and a real downside.
Examples:

- *Chaos Inoculation* — maximum life becomes 1, immune to chaos damage. Turns a
  character into a pure energy shield build, and incidentally solves poison and
  bleed, which otherwise bypass energy shield.
- *Giant's Blood* — wield two-handed axes, maces and swords in one hand.
  **Triples** the attribute requirements of martial weapons, and halves the
  inherent life granted by strength. Two downsides, both severe.
- *Necromantic Talisman* — all amulet bonuses apply to your minions instead of
  you.

Keystones are worth checking early. "What keystone does this build want, and
what is the path to it" is a better opening question than "which small passives
are good". All 33 are listed with their downsides in
[14-keystones.md](14-keystones.md).

## Points

See [00-progression-and-points.md](00-progression-and-points.md) for the budget.
Short version: **1 point per level after 1, plus 24 from quests.**

Respeccing costs **gold**, paid to The Hooded One or Doryani in town. It is not
free, but it is not permanent either — advice that turns out wrong is
recoverable.

## Weapon set passive points

A separate pool. **24 weapon set passive points** by the end of the campaign,
allocated to nodes that only apply while a given weapon set is equipped.

Swapping weapons swaps which set of allocations is live, automatically. So a
character can carry, say, 24 points of cold scaling on set 1 and 24 points of
lightning scaling on set 2, and switch between two damage types without
respeccing.

This is easy to forget when totalling a build's investment.

## Ascendancies

- **3 ascendancies per class, 36 total.**
- Each has a small tree with **8 allocatable points**.
- The bonuses are usually the reason to pick one class over another, so a build
  recommendation that ignores the ascendancy is missing the point of the build.

## Instilled notables (amulet anoints)

The exception the user flagged: **an amulet can grant a notable passive the
character has not allocated.**

- **Liquid Emotions** drop from Delirium encounters in Waystones, and in
  Simulacrums.
- Three of one tier reforge into one of the next tier, at the **Reforging Bench**
  (unlocked by completing the Molten Vault in Act 3).
- Instilling happens at **The Withered Willow**, via the Decanter of Madness: one
  amulet plus three Liquid Emotions.
- **Most notables can be instilled**, but not all.
- To find the combination for a notable: open the tree, hover the notable, press
  **Alt**.
- **One instil per amulet.** Repeating the process overwrites the previous one,
  at any time.
- The instilled notable shows as allocated on the tree, in full colour.

The game calls it "instil"; players call it "anoint", carried over from PoE 1.

### How PoB handles this

PoB already models it. Instilled notables land in
`build.calcsTab.mainEnv.grantedPassives`, keyed by node id, and PoB's own
calculation code checks that table everywhere it counts node modifiers
(`CalcsTab.lua`, `PassiveTreeView.lua`, `TreeTab.lua`).

The bridge respects this: `node_info` returns `already_granted = true` for a
granted node, and path planning skips granted nodes when scoring, because
allocating a node you already have grants nothing.

**Practical consequence:** a notable can be active without a passive point spent
on it. Do not tell someone to path to a notable their amulet already grants, and
do not count it against their point budget.

## Pathing rules

- **Every allocated node must connect** to the character's start through other
  allocated nodes. There is no allocating an island.
- Each class starts at its own fixed node, which determines what is cheap and
  what is far.
- Ascendancy nodes are a separate connected sub-tree with its own start.
- Instilled notables are the **only** exception to needing a path — and they do
  not create connectivity for neighbouring nodes.

The bridge validates connectivity in `alloc_trace` rather than trusting the
caller, so a disconnected path is rejected instead of silently applied.

## Advising on the tree

1. **Work from notables and keystones**, not small passives.
2. **Respect the point budget** for the character's level.
3. **Set attribute node choices explicitly** when the request names an attribute
   — allocating the node leaves the choice unset.
4. **Check whether a notable is already granted** by the amulet before pathing
   to it.
5. Remember **weapon set points are a separate pool** of 24.
6. Mention respec cost when suggesting a large change — it is gold, not free.

## Sources

Mobalytics guides: passive-skill-tree, distilled-emotions, beginner-guide. PoB
behaviour verified against `CalcsTab.lua` and `crates/pob-engine/lua/bridge.lua`.
