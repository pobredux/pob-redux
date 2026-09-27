# Skills and gems

Written against 0.5.5. This is the file that fixes bad gem advice.

## The problem this solves

PoB's gem data has **no character level requirement**. `grantedEffect.levels[i].
levelRequirement` is 0 for nearly every gem, and there is no lookup table
anywhere in the PoB source. The only signal PoB carries is a `Tier` number, which
`list_gems` exposes.

So without this file, recommending gems for a level 21 character is guesswork —
and it showed. The mapping below closes that gap.

## Skill gem tier to character level

The authoritative table, from poe2db. It also gives the **base support socket
count**, which scales with tier.

| Tier | Requires level | Support sockets |
|---|---|---|
| 1 | 1 | 2 |
| 2 | 3 | 2 |
| 3 | 6 | 2 |
| 4 | 10 | 2 |
| 5 | 14 | 2 |
| 6 | 18 | 2 |
| 7 | 22 | 2 |
| 8 | 26 | 2 |
| 9 | 31 | 2 |
| 10 | 36 | **3** |
| 11 | 41 | 3 |
| 12 | 46 | 3 |
| 13 | 52 | 3 |
| 14 | 58 | 3 |
| 15 | 64 | **4** |
| 16 | 66 | 4 |
| 17 | 72 | 4 |
| 18 | 78 | 4 |
| 19 | 84 | 4 |
| 20 | 90 | **5** |

This table was first derived here by cross-referencing PoB's `Tier` field against
the `level_interval` values in real build-planner files. Every derived value —
tiers 1, 3, 4, 5, 7, 8, 9, 11, 13 and 14 — matches poe2db exactly, and poe2db
fills the gaps and extends the ladder to tier 20.

**Support sockets scale with gem tier.** A gem is not simply "found with 2
sockets": tiers 1–9 have 2, tiers 10–14 have 3, tiers 15–19 have 4, and tier 20
has 5. Jeweller's Orbs raise it from there. So a low-tier gem cannot hold five
supports no matter how many orbs are spent on it — the five-support setups that
[09-buildcraft.md](09-buildcraft.md) shows are standard need a tier 20 gem or
orb investment.

The skill gems actually in PoB's data cluster on tiers 1, 3, 4, 5, 7, 8, 9, 11,
13 and 14; tiers 2, 6, 10 and 12 have no skill gems, though the level ladder
defines them.

**Tier 0 skill gems** (165 of them, the largest group) are not on this ladder.
They are hidden internal skills, minion abilities, triggered effects and lineage
(unique) skills. `list_gems` cannot tell them apart, so treat a tier 0 result
with suspicion unless you recognise it.

Worked examples that anchor the table: Lightning Arrow and Explosive Grenade are
tier 1 (level 1). Snipe and High Velocity Rounds are tier 3 (level 6). The
heralds and Ghost Dance are tier 4 (level 10). Gas Grenade and Electrocuting
Arrow are tier 5 (level 14). Temporal Chains and Voltaic Mark are tier 7 (level
22). Ice Shot and Despair are tier 9 (level 31). Tornado Shot is tier 11 (level
41).

**How to use it.** Call `list_gems`, then drop anything whose tier maps above the
character's level. A level 21 character cannot use Ice Shot, no matter how well
it fits the build.

## Support gem tiers

Supports use tiers **1 to 5** only, matching the level of the Uncut Support Gem
they are cut from. Poison I is tier 1, Poison II tier 3, Poison III tier 5.

**Tier 0 supports are lineage supports** — the unique ones that drop as items
rather than being cut: Atziri's Allure, Uhtred's Omen, Kurgal's Leash, Olroth's
Conviction. They are endgame drops. Do not put them in a levelling
recommendation.

## How gems actually work

**Uncut skill gems** drop as items. Using one lets you pick any skill gem in the
game that the uncut gem's level is high enough for. Finding one early does not
unlock everything.

**Every class can use every gem**, as long as it meets the requirements. Gems are
grouped by weapon type (maces, crossbows, quarterstaves) but that grouping is
suggestion, not restriction.

**Attribute requirements gate everything.** Each skill gem has a strength,
dexterity and/or intelligence requirement that grows with its gem level —
`list_gems` returns it as `req_str`, `req_dex`, `req_int` at the gem level the
character can use (`gem_level`). The character needs the **highest single
requirement** across its items and gems, never the sum; see
[16-evaluating-changes.md](16-evaluating-changes.md) for the rule and the
tools that report it.

## Sockets and supports

- **9 skill gem slots** by default.
- Each skill gem takes **up to 5 support gems**.
- Base sockets come from the gem's tier (2 / 3 / 4 / 5 — see the table above).
  More come from **Jeweller's Orbs**.
- **Supports have no requirement of their own.** All supports of one colour
  across the build form a single requirement source of 5 each (four blue
  supports is one source of 20 Int), which counts only when it is higher than
  every item and skill gem requirement for that attribute. It is never added
  on top of a gem's own cost. Off-colour supports on a low-level character are
  where it bites; same-colour supports on a build that already meets its
  weapon's requirement change nothing.
- **The same support cannot be used twice** across your gems — check
  `list_valid_supports` rather than assuming.

Some skills are not gems at all. **A weapon or shield can grant a skill**
(Raise Shield from a shield, for instance): it appears as a socket group whose
`grantedBy` names the item and slot, cannot be removed or moved, and leaves
with the item. Its presence says nothing about whether the build uses it. Do not
count it as one of the build's chosen skills, do not propose replacing it, and
do not spend supports on it unless the build plays it.

## Spirit and persistent buffs

Persistent buffs — heralds, auras, some minion skills — **reserve spirit**
permanently. Herald of Ash reserves 30.

Spirit comes almost entirely from quests:

| After | Spirit |
|---|---|
| Act 1 | 30 |
| Act 3 | 60 |
| Interlude 3 | 100 |

**Check the build's spirit before suggesting any reserved buff.** A character
with 30 spirit can run exactly one herald and nothing else. Recommending two is
the single most common way this goes wrong.

Details worth knowing:

- Persistent buffs granted by **items** (Malice on a sceptre) reserve **no
  spirit**.
- **Persistent buff supports** reserve additional spirit of their own. Vitality I
  regenerates 1% life per second but costs 20 more spirit.
- The supported skill does **not** itself need to reserve spirit for a persistent
  buff support to work.
- **Supports with a cost multiplier do not raise spirit reservation** unless they
  specifically state a reservation multiplier.
- Persistent buffs are engraved from **Uncut Spirit Gems**, and must be activated
  manually in the skill panel (default G) — they are not on by default.

## Meta gems

Skill gems that hold other skill gems. Mostly triggers — Cast on Freeze, Cast on
Ignite, Cast on Minion Death.

- They run on **Energy**: the gem gains energy when its condition is met, and
  triggers everything socketed when energy fills. Maximum energy is the combined
  base cast time of the socketed spells, so slower spells need more energy.
- Energy resets to 0 after triggering.
- **You still pay the mana cost** of the triggered skills.
- They **reserve spirit** (Barrier Invocation reserves 60) and count as
  persistent buffs, so they can take persistent buff supports if you have the
  spirit spare.
- The 5 sockets can hold any mix of skills to trigger and supports to buff them —
  five triggered skills, or three skills and two supports.

**Enemy power** drives some trigger conditions:

| Rarity | Multiplier |
|---|---|
| Normal | 1 |
| Magic | 2 |
| Rare | 5 |
| Unique | always 20 power |

Base power is 0.5 for the weakest monsters, 2–3 for stronger ones, multiplied by
the rarity value.

## Attacks vs spells

- **Attacks** take base damage from the **weapon**, and base critical chance from
  the weapon. Weapon quality dominates their scaling.
- **Spells** take base damage and base critical chance from the **gem**. Wands
  and similar have no attack stats — they carry spell damage, gem levels, cast
  speed and crit instead.
- **Damage over time** scales mostly off the gem, like spells.

This decides what gear advice to give. Telling an attack build to chase spell
damage, or a spell build to upgrade its weapon's physical damage, is wasted
guidance.

## Checklist before recommending gems

1. **Set the character level first** — `set_level`. Everything below depends on
   it.
2. Call **`list_gems`** and filter by the tier table above.
3. Check **attribute requirements** with `build_summary.requirements` after
   adding: the highest single source per attribute, never a sum.
4. Check **spirit** before any herald, aura or meta gem.
5. Use **`list_valid_supports`** for the actual legal supports on that skill,
   rather than naming supports from memory.
6. Prefer a **coherent damage type**: one skill's supports should all push the
   same scaling. Four unrelated skills with scattered supports is the failure
   mode to avoid.

## Sources

Mobalytics guides: meta-gems, persistent-buffs, beginner-guide. Tier-to-level
mapping derived from PoB's gem data cross-referenced against build-planner files
in `Documents/My Games/Path of Exile 2/BuildPlanner`.
