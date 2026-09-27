# Keywords

Short definitions of PoE1 keywords a player meets in gem, item and passive text. Written against 3.29 (PoB 2.67.2,
tree 3_29). Numbers are PoB's unless a line says poedb. Bigger topics have their own files:
[01-defences.md](01-defences.md), [02-damage.md](02-damage.md), [03-ailments.md](03-ailments.md).

## Debuffs on enemies

**Blind** — 20% less accuracy and 20% less evasion. poedb gives a base duration of 4 seconds.

**Maim** — 30% reduced movement speed. poedb: 4 seconds; maims do not stack with each other.

**Hinder** — reduced movement speed, 30% from most sources (poedb). Only the strongest hinder applies. A target can
be hindered and maimed at once. PoB tracks hinder only as a condition.

**Intimidate** — the target takes 10% increased attack damage. poedb: usually 4 seconds.

**Unnerve** — the target takes 10% increased spell damage. poedb: usually 4 seconds.

**Taunt** — the target attacks the taunter and deals 10% less damage to anyone else (poedb, 3 seconds). PoB tracks
it only as a condition.

**Withered** — 6% increased chaos damage taken per stack, up to **15 stacks** (90%).

**Exposure** — ‑10% to one elemental resistance by default. Only the strongest exposure of each element counts;
exposures of different elements all apply. Exposure adds to curses. poedb: 4 seconds by default.

**Impale** — stores **10%** of a physical hit's damage before mitigation. Each impale deals that damage again when
the target is hit, for **5 hits** or **8 seconds**. With 5 impales active, PoB adds 50% of the stored hit per hit.
Enemy armour and physical damage reduction apply to it.

**Debilitate** — 10% less damage dealt and 20% less movement speed.

**Crush** — ‑15% physical damage reduction.

## Buffs on you

**Onslaught** — 20% increased attack, cast and movement speed.

**Fortification** — 1% less damage taken from hits per stack. Base maximum **20** stacks. PoB uses a 6-second base
duration.

**Elusive** — at full effect, 15% chance to avoid all damage from hits and 30% increased movement speed. The
effect falls by 20% per second until it reaches 0. PoB averages it to 50% effect (7% avoid) unless configured.
poedb: you cannot gain Elusive again until it expires.

**Unholy Might** — converts all physical damage to chaos and lets your hits wither. poedb adds a 25% chance to
wither on hit.

**Chaotic Might** — 30% of physical damage gained as extra chaos damage.

**Arcane Surge** — 20% increased cast speed and 30% increased mana regeneration rate for 4 seconds. The Arcane
Surge support also gives more spell damage while it lasts: 14% at gem level 1, 35% at level 20. poedb lists 10%
more spell damage at level 1; the app uses PoB's value.

**Adrenaline** — 100% increased damage, 25% increased attack, cast and movement speed, 10% additional physical
damage reduction.

**Tailwind** — 8% increased action speed.

**Rage** — each Rage gives **1% more attack damage**. The maximum is **30**. You lose 10 Rage per second after 2
seconds without gaining Rage.

**Consecrated ground** — regenerate 5% of life per second while standing in it.

## Charges

| Charge | Base maximum | Bonus per charge |
|---|---|---|
| Endurance | 3 | 4% physical and 4% elemental damage reduction |
| Frenzy | 3 | 4% increased attack and cast speed, 4% more damage |
| Power | 3 | 50% increased critical strike chance |

Charges last **10 seconds** by default. `set_config` decides whether PoB counts them as active.

## States

**Low life** — 50% of maximum life or less. **Full life** — 100%. PoB treats a character with at least 50% of its
life reserved as on low life.

**Immune** — the effect cannot apply. **Chance to avoid** at 100% has the same result.

**Overcapped** — resistance or block chance above its maximum. It does nothing until something lowers the value.

## Damage terms

**Hit** — one instance of damage from an attack or spell. Only hits can crit, be evaded, be blocked or be
suppressed. Armour and penetration apply only to hits.

**Damage over time** — damage dealt each second for a duration: skill effects such as burning ground, and the
ailments ignite, bleed and poison. It cannot crit, be evaded or be blocked. Penetration does not apply.

**Degen** — life, mana or energy shield lost each second, such as Righteous Fire's self-burn or standing in
burning ground. It is damage over time on you.

**Culling strike** — kills an enemy at **10%** life or less. PoB multiplies `CombinedDPS` by 1.11 for it.

**Increased / more** — all "increased" modifiers add together; each "more" multiplies. See
[02-damage.md](02-damage.md).

**Penetration** — a hit ignores part of the enemy's resistance. It does nothing for damage over time.

**Conversion / gained as extra** — damage changes type along physical → lightning → cold → fire → chaos.
Converted damage keeps the modifiers of every type it passed through.

## Recovery terms

**Leech** — recovers a share of the damage you deal, over time. Each instance recovers 2% of the maximum pool per
second. Total life and mana leech caps at 20% of the maximum per second; energy shield leech at 10%. One instance
recovers at most 10% of the pool. poedb: leech normally ends when the pool is full. Vaal Pact makes melee life leech
instant.

**Recoup** — a share of the damage you take from hits is recovered over **4 seconds** (3 seconds with some
modifiers).

**Recharge** — energy shield refilling at 33.3% per second after 2 seconds without taking damage. See
[01-defences.md](01-defences.md).

**Ward** — a pool that takes hit damage before energy shield. PoB uses a 2-second restore delay.

**Guard skill** — a buff, such as Steelskin, that absorbs part of each hit before ward and energy shield.

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/Modules/CalcPerform.lua` (Onslaught, Arcane Surge, Unholy Might, Chaotic
  Might, Tailwind, Adrenaline, consecrated ground, Elusive, Withered, Blind, Rage, fortification, exposure),
  `Modules/CalcSetup.lua` (maim, intimidate, unnerve, debilitate, crush, charges, rage and fortification
  maximums), `Modules/CalcOffence.lua` (impale, culling multiplier, leech, penetration), `Modules/CalcDefence.lua`
  (pool order, recoup), `Modules/ConfigOptions.lua` (exposure, intimidate, unnerve, withered, taunt and hinder
  conditions), `Modules/ModParser.lua` (culling strike, Unholy Might), `Modules/Data.lua` (`data.misc`),
  `Data/Misc.lua` (`characterConstants`), `Data/Skills/sup_int.lua` (Arcane Surge support),
  `TreeData/3_29/tree.json` (Vaal Pact).
- https://poedb.tw/us/Blind, https://poedb.tw/us/Maim, https://poedb.tw/us/Hinder, https://poedb.tw/us/Taunt,
  https://poedb.tw/us/Intimidate, https://poedb.tw/us/Withered, https://poedb.tw/us/Impale,
  https://poedb.tw/us/Exposure, https://poedb.tw/us/Fortification, https://poedb.tw/us/Elusive,
  https://poedb.tw/us/Unholy_Might, https://poedb.tw/us/Arcane_Surge, https://poedb.tw/us/Rage,
  https://poedb.tw/us/Recoup, https://poedb.tw/us/Leech, https://poedb.tw/us/Onslaught
- https://maxroll.gg/poe/resources/defenses-and-defensive-layering (Steelskin as a guard skill)
