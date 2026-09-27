# Defences

Written against 3.29 (PoB 2.67.2, tree 3_29). Every number comes from PoE1 PoB's data and Calc modules unless the
line names another source. The app calculates with PoB, so PoB's numbers win. Caps and constants are also in
[13-game-constants.md](13-game-constants.md).

## Where a hit goes

PoB runs every enemy hit through this chain (`CalcDefence.lua`).

1. **Avoid the hit.** Evasion (attacks only), block, dodge and "avoid all damage from hits" (Elusive). An avoided
   hit stops here.
2. **Mitigate.** Resistance first, then armour and additional physical damage reduction on what is left, then
   spell suppression and "damage taken" modifiers.
3. **Drain pools, in this order:** allies that take damage for you (Frost Shield, totems, minions), Aegis, guard
   skills, ward, **energy shield**, mana through Mind over Matter, life-loss prevention (the part taken over 4
   seconds instead), and finally **life**.

**Chaos damage bypasses energy shield** and goes straight to life. PoB sets chaos bypass to 100% unless a modifier
says chaos damage does not bypass. Chaos Inoculation sets maximum life to 1 and makes you immune to chaos damage.

Damage over time skips step 1. Evasion, block, suppression and armour do nothing against it.

## Life

| Source | Amount in PoB |
|---|---|
| Base | 38 + **12 per level** (1,238 at level 100) |
| Strength | **+1 per 2 strength** |
| Low life | 50% of maximum life or less |

In our PoE1 corpus, the 155 life-based ladder characters have a median of **5,318 life** (4,348 to 6,407).

## Energy shield

- Recharge restores **33.3% of maximum energy shield per second** (2,000% per minute).
- Recharge starts after **2 seconds** without taking damage. Any damage restarts the timer.
- "Faster start of energy shield recharge" divides the delay: 100% faster means 1 second.
- Energy shield recovery rate scales recharge.
- PoB adds 1% increased energy shield per 10 intelligence.
- While you have more energy shield than the hit, you have a 50% chance to avoid being stunned.

Keystones that change recharge:

| Keystone | Effect |
|---|---|
| Eldritch Battery | Energy shield protects mana instead of life; 50% less recharge rate |
| Wicked Ward | Recharge is not interrupted if it began recently; 40% less recharge rate |
| Ghost Reaver | Cannot recharge; leech energy shield instead of life |
| Zealot's Oath | Life regeneration applies to energy shield instead |
| Divine Shield / Ghost Dance | Cannot recover energy shield above armour / evasion |

## Armour

PoB's formula (`calcs.armourReductionF`):

```
reduction = armour / (armour + 5 × hit damage)
```

| Reduction | Armour needed |
|---|---|
| 25% | 1.67 × the hit |
| 50% | **5 × the hit** |
| 75% | 15 × the hit |
| 90% | 45 × the hit |

- Armour plus additional physical damage reduction is capped at **90%**. Imbalanced Guard caps every type at 50%.
- Each endurance charge gives **4% physical and 4% elemental damage reduction**.
- Armour applies to physical hits only. A modifier such as "X% of armour applies to fire damage" extends it.
- Armour is good against many small hits and weak against one large hit. 20,000 armour stops 50% of a
  4,000 hit and 20% of a 16,000 hit.

## Evasion

PoB's enemy chance to hit (`calcs.hitChance`), clamped between 5% and 100%:

```
chance to hit = 125 × accuracy / (accuracy + (evasion / 5) ^ 0.9)
```

Your chance to evade is 100% minus that, capped at **95%**. Evasion works against **attacks only**.

A level-84 monster (PoB's default pinnacle boss level) has 538 accuracy:

| Evade chance | Evasion needed |
|---|---|
| 25% | 3,448 |
| 50% | 8,488 |
| 75% | 25,242 |
| 90% | 81,607 |

- Base evasion is 15. Each 5 dexterity gives 1% increased evasion rating.
- PoB lowers the enemy's critical strike chance by your evade chance, so evasion also reduces crits taken.
- Maxroll says evasion uses an **entropy** system: you rarely take several hits in a row, but you are always hit
  eventually. PoB does not model entropy; it uses the average evade chance.
- Iron Reflexes converts all evasion to armour.

## Block

| Type | Base maximum | Absolute cap |
|---|---|---|
| Attack block | **75%** | 90% |
| Spell block | **75%** | 90% |

- A shield gives base attack block. Dual wielding gives 20% attack block.
- Spell block needs its own modifiers.
- **Glancing Blows** doubles both block chances. You take 65% of the damage from blocked hits.
- **Versatile Combatant** gives ‑10% to both maximums and 2% spell block per 1% of overcapped attack block.
- Maxroll says a blocked hit also applies no ailments, and guard skills do not lose absorption on a blocked hit.

## Spell suppression

- Chance caps at **100%**.
- A suppressed spell deals **40% less damage** (`SuppressionEffect = 40`). poedb says the same and adds that it
  also applies to ailments from that hit.
- Maxroll's beginner pages say 50%. That is older; the app uses 40%.
- At exactly 100% PoB treats suppression as a fixed reduction. Below 100% it averages it.
- Acrobatics turns suppression into spell dodge at 50% of its value, up to 75%.

## Resistances

| Constant | Value |
|---|---|
| Base maximum (fire, cold, lightning, chaos) | **75%** |
| Absolute maximum | **90%** |
| Floor | ‑200% |
| Campaign penalty | ‑30% after Act 5, **‑60% after Act 10** (PoB default) |

- The penalty also applies to chaos resistance.
- PoB truncates fractional resistance.
- Overcapped resistance protects against curses and exposure. PoB's default pinnacle boss hits you with 3%
  elemental penetration.

## Mind over Matter

The keystone takes **40% of damage from mana before life**. PoB caps the mana it can use:

```
usable mana = unreserved life × 0.4 / 0.6   (two thirds of life)
```

A character with 5,000 unreserved life gains nothing from mana above 3,333. Only unreserved mana counts. With
Eldritch Battery, energy shield protects that mana. Smaller sources ("X% of damage is taken from mana before
life") add to the 40%.

## What ladder builds stack

In our PoE1 corpus (327 ladder characters, level 96 to 100):

| Pool | Characters | Median pool |
|---|---|---|
| Life, energy shield below 20% of life | 155 (47.4%) | 5,318 life |
| Hybrid, energy shield 20% to 100% of life | 91 (27.8%) | |
| Chaos Inoculation | 67 (20.5%) | 9,009 energy shield |
| Energy shield above life, no CI | 14 (4.3%) | |

| Layer | Characters |
|---|---|
| All three elemental resistances at 75%+ | 266 (81.3%) |
| At least one elemental resistance at 80%+ | 209 (63.9%) |
| Chaos resistance 75%+ | 154 (47.1%) |
| Armour 10,000+ | 143 (43.7%) |
| Attack block 50%+ / spell block 50%+ | 142 (43.4%) / 143 (43.7%) |
| Spell suppression 100% | 85 (26.0%); 43 of 83 hardcore (51.8%) |
| Evasion 10,000+ | 74 (22.6%) |
| Mind over Matter | 42 (12.8%) |

The most common combinations are armour alone (41), armour with both blocks (26) and evasion with 100%
suppression (21). Only 11 non-CI characters (3.4%) have negative chaos resistance.

## Reading EHP in PoB

| Field | Meaning |
|---|---|
| `TotalEHP` | Average number of hits to kill you × damage per hit. Includes evade, block, suppression and avoidance. |
| `PhysicalMaximumHitTaken`, `FireMaximumHitTaken`, `ColdMaximumHitTaken`, `LightningMaximumHitTaken`, `ChaosMaximumHitTaken` | Largest single hit of that type you survive from full pools. Counts resistance, armour, reduction, pools and MoM, and suppression only at 100%. Ignores evade, block and avoidance. |
| `SecondMinimalMaximumHitTaken` | Second-lowest of the five ("Eff. Maximum Hit Taken"). |
| `EHPSurvivalTime` | Hits to die × enemy attack time (700 ms default). |

- The enemy comes from `set_config` `enemyIsBoss`. The default is a pinnacle boss: level 84, **2,241 damage per
  element and physical, 896 chaos**.
- `enemyDamageType` picks the damage type. "Average" mixes all typed damage.
- "EHP calc unlucky" rolls block, suppression and avoidance as unlucky.

In our PoE1 corpus the median ladder `TotalEHP` is **101,759** (45,485 to 186,237). The median
`PhysicalMaximumHitTaken` is **15,473** (8,856 to 25,016).

| Pool | Median EHP | Median physical max hit |
|---|---|---|
| Chaos Inoculation | 158,275 | 20,119 |
| Life-based | 91,641 | 18,033 |
| Hybrid or energy shield | 69,630 | 11,638 |

## Advising

1. Check resistances in `build_summary`. Fix any elemental resistance below 75% first.
2. Read the five `…MaximumHitTaken` values in `get_stats`. The lowest one is the type that kills the character.
3. Find the missing layer before adding more of an existing one. A 5,000-life character with no block, no
   suppression and 2,000 armour has room in every layer.
4. For MoM, check that unreserved mana reaches two thirds of unreserved life.
5. Compare EHP only under the same `enemyIsBoss` and `enemyDamageType`.

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/Modules/CalcDefence.lua` (`calcs.hitChance`, `calcs.armourReductionF`,
  `calcs.reducePoolsByDamage`, block, suppression, energy shield recharge, bypass, Mind over Matter, EHP and
  maximum hit), `Modules/CalcSetup.lua` (base life, mana, evasion, resistances, block maximums, charges),
  `Modules/CalcPerform.lua` (attribute bonuses), `Modules/Data.lua` (`data.misc`), `Data/Misc.lua`
  (`characterConstants`, `monsterAccuracyTable`, `monsterDamageTable`), `Modules/ConfigOptions.lua`
  (`enemyIsBoss`, `resistancePenalty`, `EHPUnluckyWorstOf`), `TreeData/3_29/tree.json` (keystone text).
- Corpus: `S:\_projects_\_poe2_\corpus\poe1\index.json`, `stages.jsonl` (327 usable ladder characters).
- https://poedb.tw/us/Spell_Suppression
- https://maxroll.gg/poe/resources/defenses-and-defensive-layering
- https://maxroll.gg/poe/getting-started/defenses-for-beginners
