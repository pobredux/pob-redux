# Game constants

Hard numbers from PoE1 Path of Building: `data.characterConstants` and `data.monsterConstants` (read from the
game's `Character.ot` and `Monster.ot`), `data.misc`, and the Calc modules that use them. The app calculates with
these numbers, so **this file wins for anything the app reports**.

Written against 3.29 (PoB 2.67.2, tree 3_29).

## Attributes

| Attribute | Inherent bonus | Rounding |
|---|---|---|
| Strength | **+1 maximum life per 2** | rounded down |
| Strength | **+1% increased melee physical damage per 5** | rounded down |
| Dexterity | **+2 accuracy rating per point** | none |
| Dexterity | **+1% increased evasion rating per 5** | rounded down |
| Intelligence | **+1 maximum mana per 2** | rounded down |
| Intelligence | **+1% increased maximum energy shield per 10** | rounded down |

PoB rounds each bonus down (`m_floor`), so 49 strength gives 24 life and 9% melee physical damage.

Several keystones change these bonuses:

- **Iron Grip** adds the strength damage bonus to projectile attacks. **Iron Will** adds it to spells.
- **Iron Reflexes** and **Magebane** remove the dexterity bonus to evasion.
- **Solipsism** removes the intelligence bonus to energy shield.
- **Supreme Ostentation** (Elegant Hubris) removes every inherent attribute bonus.

**Characters gain no attributes from levelling.** `strength_per_level`, `dexterity_per_level` and
`intelligence_per_level` are all 0.

### Starting attributes

| Class | Str | Dex | Int |
|---|---|---|---|
| Marauder | 32 | 14 | 14 |
| Ranger | 14 | 32 | 14 |
| Witch | 14 | 14 | 32 |
| Duelist | 23 | 23 | 14 |
| Templar | 23 | 14 | 23 |
| Shadow | 14 | 23 | 23 |
| Scion | 20 | 20 | 20 |

## Per level

| Stat | Formula in PoB | Level 1 | Level 100 |
|---|---|---|---|
| Base maximum life | 38 + **12 per level** | 50 | 1,238 |
| Base maximum mana | 34 + **6 per level** | 40 | 634 |
| Base accuracy rating | **2 per level** above 1 | 0 | 198 |
| Base evasion rating | flat **15** | 15 | 15 |
| Passive points | 1 per level above 1 | 0 | 99 |

Evasion has no per-level gain. PoB dropped the old 3-per-level evasion in July 2021.

PoB counts **23 quest passive points** over the campaign (`Build.lua` act table). The bandit choice adds 1 more
if the player kills all three bandits. In PoB's count the most a character can have is 99 + 23 + 1 = **123
points**, plus 8 ascendancy points. `build_summary` returns the budget as `pointsAvailableMin` and `pointsAvailableMax`.

## Resistances

| Constant | Value |
|---|---|
| Base maximum resistance (fire, cold, lightning, chaos) | **75%** |
| Absolute maximum resistance | **90%** |
| Resistance floor | ‑200% |
| Totem base resistances | +40% elemental, +20% chaos |

### Campaign penalty

The penalty applies to fire, cold, lightning **and chaos**.

| Campaign progress | Penalty | `set_config` `resistancePenalty` |
|---|---|---|
| Before Act 5 is finished | 0% | `0` |
| After Act 5 | **‑30%** | `-30` |
| After Act 10 (PoB default) | **‑60%** | `-60` |

PoB defaults to ‑60%. When advising a character that is still in the campaign, set the penalty that matches
its act first. Left at the default, a campaign build looks 30 to 60 points short on every resistance.

## Caps

| Cap | Value | Where |
|---|---|---|
| Physical damage reduction | **90%** | `maximum_physical_damage_reduction_%` |
| Chance to block attack damage, base maximum | **75%** | `maximum_block_%` |
| Chance to block spell damage, base maximum | **75%** | `base_maximum_spell_block_%` |
| Chance to block, absolute cap | **90%** | `BlockChanceCap` |
| Chance to suppress spell damage | **100%** | `SuppressionChanceCap` |
| Damage prevented by a suppressed spell | **40%** | `SuppressionEffect` |
| Chance to evade | **95%** | `EvadeChanceCap` |
| Enemy chance to hit you, minimum | 5% | `calcs.hitChance` |
| Chance to dodge spell hits (Acrobatics only) | 75% | `SpellDodgeChanceMax` |
| Chance to avoid damage (elemental, projectiles, all hits) | 75% | `AvoidChanceCap` |
| Action speed reduction from Temporal Chains effects | 75% | `TemporalChainsEffectCap` |

**Spell suppression prevents 40% of the damage.** PoB changed `SuppressionEffect` from 50 to 40 in October 2025,
and poedb's item text also says 40%. A source that says 50% predates the change.

PoB applies **no movement speed cap**. Maim gives ‑30% movement speed.

## Recovery and leech

| Constant | Value |
|---|---|
| Energy shield recharge rate | **33.3% of maximum per second** (2,000% per minute) |
| Energy shield recharge delay | **2 seconds** |
| Ward restore delay | 2 seconds |
| Base mana regeneration | **1.75% of maximum mana per second** (105% per minute) |
| Leech rate per instance | 2% of the maximum pool per second |
| Maximum life leech rate | **20% of maximum life per second** |
| Maximum mana leech rate | 20% of maximum mana per second |
| Maximum energy shield leech rate | 10% of maximum energy shield per second |
| Largest single leech instance | 10% of the maximum pool |
| **Low life** | **50% of maximum life or less** |
| Full life | 100% |

PoB treats a character as on low life when at least 50% of its life is reserved. Chaos Inoculation sets life to 1
and always counts as full life.

## Charges

| Charge | Base maximum | Bonus per charge |
|---|---|---|
| Endurance | **3** | **4% physical damage reduction** and **4% elemental damage reduction** |
| Frenzy | **3** | **4% increased attack speed**, **4% increased cast speed**, **4% more damage** |
| Power | **3** | **50% increased critical strike chance** |

Charges last **10 seconds** by default (`ChargeDuration`). Other limits from the same data:

| Thing | Limit |
|---|---|
| Inspiration charges | 5 |
| Rage | 30 |
| Fortification | 20, each ‑1% damage taken from hits |
| Totems | 1 |
| Traps / remote mines | 15 / 15 |
| Brands attached | 3 |
| Curses on an enemy | 1 |
| Impale | 5 hits, 8 seconds |

## Hits, crits and ailments

| Constant | Value |
|---|---|
| Base critical strike multiplier | **150%** |
| Extra damage over time multiplier for ailments from critical strikes | +50% |
| Dual wielding | 10% more attack speed, +20% chance to block attacks |
| Armour | reduction = armour / (armour + **5** × hit damage), cap 90% |
| Bleeding | **70%** of the physical hit per second, **5 seconds** |
| Bleeding against a moving enemy | 200% more damage |
| Poison | **30%** of physical and chaos hit per second, **2 seconds** |
| Ignite | **90%** of the fire hit per second, **4 seconds** |
| Impale | stores 10% of the physical hit |

Armour uses a factor of 5 in PoE1. A hit equal to your armour value is reduced by about 17%. Armour five times
the hit reduces it by 50%.

## Enemy defaults

`set_config` var `enemyIsBoss` sets PoB's assumed target. **PoB defaults to `Pinnacle`.**

| `enemyIsBoss` | Level | Elemental res | Chaos res | Armour / evasion | Other |
|---|---|---|---|---|---|
| `None` | character level, max 85 | 0% | 0% | level table | none |
| `Boss` | character level, max 85 | **40%** | **25%** | level table | none |
| `Pinnacle` (default) | **84** | **50%** | **30%** | 150% / 125% of table | hits you with 3% penetration |
| `Uber` | **85** | **50%** | **30%** | 125% / about 117% of table | **takes 70% less damage**, 8% penetration |

All boss settings mark the enemy as rare or unique. They also raise its ailment threshold: 488% more for `Boss`
and 404% more for `Pinnacle` and `Uber`.

Other monster constants:

| Constant | Value |
|---|---|
| Monster base maximum resistance | 75% |
| Monster physical damage reduction cap | 75% |
| Monster critical strike multiplier | 130% |
| Highest enemy level PoB uses | 85 |

Enemy resistance set above 75% also raises the enemy's maximum, up to 90%. The config box "Enemy Max Resistance is
always 75%" (`enemyMaxResist`) stops that.

PoB's enemy-level tooltip says normal enemies and standard bosses default to level 83. Its code
(`ConfigTab:UpdateLevel`, `CalcSetup`) uses the character level capped at 85. Use the code's rule; set
`enemyLevel` if the level matters.

## Using these numbers

1. Before judging resistances on a campaign character, set `resistancePenalty` to the act it has reached.
2. Before judging damage, check `enemyIsBoss`. The default pinnacle boss has 50% elemental resistance, so
   penetration and exposure matter more there than on a map pack.
3. When a guide and PoB disagree, state both and say which one the app uses.

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/Data/Misc.lua` (`characterConstants`, `monsterConstants`),
  `Modules/Data.lua` (`data.misc`, boss tooltip), `Modules/CalcSetup.lua` (`initModDB`, base mods, bandit and
  pantheon mods), `Modules/CalcPerform.lua` (attribute bonuses, low and full life, action speed),
  `Modules/CalcDefence.lua` (resistance, block, suppression, evasion, armour, movement),
  `Modules/CalcOffence.lua` (enemy resistance cap, leech), `Modules/ConfigOptions.lua` (`resistancePenalty`,
  `enemyIsBoss`, `enemyLevel`), `Classes/ConfigTab.lua` (`UpdateLevel`), `Modules/Build.lua` (quest points),
  `TreeData/3_29/tree.json` (class attributes).
- PoB git history in `../PathOfBuilding`: `fb77d892a` (suppression effect 40%), `ca73ae260` (flat base evasion).
- poedb: https://poedb.tw/us/Spell_Suppression (40% prevention text).
