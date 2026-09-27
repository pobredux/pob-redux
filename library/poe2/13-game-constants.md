# Game constants

Hard numbers read out of PoB's own `data.misc`, `data.characterConstants` and
`data.gameConstants`. These are what the app calculates with, so where a guide
disagrees, **this file wins for anything the app reports**.

Verified against the vendored PoB at `src-tauri/resources/pob` on 2026-09-03.

## Attributes

| Attribute | Inherent bonus per point |
|---|---|
| Strength | **+2 maximum life** |
| Dexterity | **+6 accuracy rating** |
| Intelligence | **+2 maximum mana** |

Attributes grant nothing else. No damage, no defence, no hidden scaling —
`CalcPerform.lua` adds exactly these three modifiers and nothing more.

**Characters gain no attributes from levelling.** `strength_per_level`,
`dexterity_per_level` and `intelligence_per_level` are all **0**. Every point of
every attribute comes from the passive tree, gear, or quest rewards.

That single fact explains the biggest number in
[09-buildcraft.md](09-buildcraft.md): 35% of a published tree is "+5 to any
Attribute" travel nodes because there is no other source of bulk attributes. Those
nodes are not dead weight — 20 points of travel nodes into strength is +200 life.

Two modifiers change this and both matter:

- **Doubled inherent attribute bonuses** doubles all three. Gemling Legionnaire
  has this, which is a large part of why it is the most-published ascendancy in
  the dataset (8 of 63 guides).
- **Halves life from strength** reduces strength to +1 life per point. Giant's
  Blood carries it.

### Conflict worth knowing

poe2db states **+8 accuracy per dexterity**. PoB and Mobalytics both say **6**,
and PoB's `AccuracyPerDexBase = 6` is what the app's numbers come from. Use 6
when explaining a PoB figure. If a future patch changes it, PoB's constant is the
thing to re-check.

## Per level

| Gain per level | Value |
|---|---|
| Maximum life | **+12** |
| Maximum mana | **+4** |
| Accuracy rating | **+6** |
| Attributes | **0** |
| Passive points | +1 |

## Caps

| Cap | Value |
|---|---|
| Physical damage reduction (all sources) | 90% |
| Maximum resistance | 90% |
| Base maximum resistance | 75% |
| **Resistance floor** | **‑200%** |
| Chance to evade | 95% |
| Chance to deflect | 95% |
| **Chance to block** | **90%** |
| Chance to dodge | 75% |
| Chance to avoid | 75% |
| Temporal Chains effect | 75% |
| Buff expiration slow | 25% |
| Quality (default max) | 20% |

The **resistance floor of ‑200%** is the practical limit on curses and exposure
stacking. Below that, further reduction does nothing.

## Defence mechanics

| Constant | Value |
|---|---|
| Armour ratio (the `10` in the armour formula) | 10 |
| Base evasion rating | 7 |
| Deflect damage reduction | 40% |
| Energy shield recharge rate | 12.5% per second |
| Energy shield recharge delay | 4 seconds |
| **Life recharge** (where it exists) | 12.5% per second after 4 seconds |
| Base mana regeneration | 4% per second (240% per minute) |
| Base leech rate | 2% per second |
| **Low resource threshold** | **35%** |
| Base block angle | **210 degrees** |
| Base fortification maximum | 20 |
| Damage taken per fortification | ‑1% |

Two of these are not in any guide and change advice:

**"Low life" is 35%**, not half. Every modifier that keys off low life, and Pain
Attunement's 30% more critical damage bonus, trigger at 35%.

**Block only covers a 210-degree arc.** Attacks from behind cannot be blocked
regardless of block chance. A block-stacking build still has a back.

## Accuracy falloff

| Constant | Value |
|---|---|
| Falloff starts at | 20 units (2 metres) |
| Falloff ends at | 90 units (9 metres) |
| Maximum penalty | **‑90% accuracy** |

Confirms the range penalty in [02-damage.md](02-damage.md). Presence radius is 40
units, so the penalty starts at half your presence and maxes at just over twice
it.

## Ailments and stun

| Constant | Value |
|---|---|
| Bleeding damage | **900% per minute = 15% per second** |
| Base bleeding duration | **5 seconds** |
| Bleeding multiplier while moving or aggravated | **×2** |
| Bloodstained-after-bleeding duration | 6 seconds |
| Base critical damage bonus | 100% |
| Unarmed base critical chance | 5% |
| Base stun duration | 500 ms |
| Base heavy stun duration | 3000 ms |
| Physical hit stun multiplier | +50% final |
| Melee hit stun multiplier | +50% final |
| **Light stun threshold per recent stun** | **+50% final** |
| Stun-count window | 4000 ms |
| Poise decay | 50% per second after 2 seconds |

**Bleed conflict:** Mobalytics says bleed magnitude is 20% of the hit's physical
damage per second. PoB says **15%** (`BleedingHitDamagePercentPerMinute = 900`),
over a **5 second** base duration. Use 15% when explaining a PoB number, and
treat the 20% figure as either outdated or a different measure.

**Stun has diminishing returns nobody documents.** Each time you are stunned
within a 4-second window, your light stun threshold rises by 50% (final). Being
chain-stunned gets progressively harder, which softens the melee-pack death
spiral described in [03-ailments.md](03-ailments.md).

## Charges

| Charge | Maximum |
|---|---|
| Power | 3 |
| Frenzy | 3 |
| Endurance | 3 |

**Charges grant no inherent benefit.** They are fuel: skills, passives and items
consume or key off them. Charges last **15 seconds** by default and gaining
another of the same type refreshes all of them.

This is different from PoE 1, where charges carried built-in bonuses. In PoE 2 a
build that generates charges without anything that spends or keys off them gains
nothing.

## Movement and actions

| Constant | Value |
|---|---|
| Base movement speed | 37 |
| Sprint bonus | +50% movement speed |
| Base presence radius | 40 units (4 metres) |
| Weapon sets | 2 |
| Weapon swap duration | 0 ms |
| Crossbow ammo switch | 300 ms |
| Server tick | 33 ms (about 30 per second) |
| Banners allowed | 1 |

**Weapon swap is instant** (0 ms). That makes the two weapon sets and their 24
passive points cheaper to use than most players assume — see
[09-buildcraft.md](09-buildcraft.md), where half the endgame builds still ignore
them.

## Monster scaling

| Constant | Value |
|---|---|
| Monster armour from strength | ×3 |
| Monster evasion from dexterity | ×2 |
| Monster base maximum resistance | 75% |
| Enemy physical damage reduction cap | 90% |

## Limits

| Thing | Limit |
|---|---|
| Stored corpses | 10 |
| Remote mines | 15 |
| Traps | 15 |
| Sigils | 3 |
| Caltrops | 20 |
| Briarpatches | 20 |
| Frost walls | 60 |

## How to use this file

When a number matters to a recommendation, prefer this file over a guide. The app
reports PoB's numbers, so quoting a guide figure that PoB disagrees with produces
advice the user cannot reproduce in the tool in front of them.

When a guide and PoB disagree and the difference changes the advice, say both and
say which one the app uses.

## Sources

`data.misc`, `data.characterConstants` and `data.gameConstants` from the vendored
PoB, plus `Modules/CalcPerform.lua` for the attribute bonus logic. Cross-checked
against poe2db's Keywords page, which is where the accuracy-per-dexterity
disagreement surfaced.
