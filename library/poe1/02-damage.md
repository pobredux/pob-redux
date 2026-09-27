# Damage

Written against 3.29 (PoB 2.67.2, tree 3_29). Formulas come from PoE1 PoB's `CalcOffence.lua` unless a line names
another source. The app reports PoB's numbers, so PoB wins where a guide disagrees.

## Damage types and conversion order

PoB orders the five types **physical → lightning → cold → fire → chaos**. Damage converts only forward along
that list. Fire can become chaos. Cold can never become lightning.

| Type | Enemy mitigates it with |
|---|---|
| Physical | Armour and physical damage reduction (enemy cap 75%) |
| Lightning, cold, fire | Matching resistance |
| Chaos | Chaos resistance |

PoB builds the conversion table per skill (`buildConversionTable`):

- **Skill conversion** (written on the gem) goes first. If skill conversion passes 100%, PoB scales it to 100% and
  drops all other conversion.
- **Other conversion** (gear, tree, buffs) fills what is left. Past 100% in total, PoB scales it down.
- **Gained as extra** adds damage of the new type and keeps the source.
- **Converted damage keeps every type it passed through.** 50% physical converted to cold is scaled by increased
  physical damage and increased cold damage, in one additive pool.

## Added, increased and more

PoB's hit damage for each type:

```
base  = weapon or gem damage × base damage multiplier + added damage × damage effectiveness
hit   = base × (1 + sum of all "increased") × product of each "more"
```

- **Attacks** take base damage from the weapon. The gem's "attack damage" percentage is the base damage multiplier.
- **Spells** take base damage from the gem. Added flat damage is scaled by the gem's damage effectiveness.
- **All "increased" and "reduced" modifiers add together.** Increased fire damage, increased spell damage and
  increased elemental damage sit in one sum.
- **Each "more" or "less" multiplies separately.** Support gems and keystones carry most of them.

At 300% increased damage, another 30% increased adds 7.5% damage. A 30% more multiplier adds 30%.

## Critical strikes

| Rule | PoB value |
|---|---|
| Base critical strike chance | Weapon for attacks, gem for spells |
| Chance formula | (base + flat added) × (1 + increased) × more |
| Chance cap | **100%** |
| Base critical strike multiplier | **150%** |
| Increased chance per power charge | **50%** |
| Damage over time multiplier for ailments from crits | **+50%** |

- PoB's average hit is `1 − c + c × multiplier`, where `c` is crit chance.
- For attacks PoB multiplies crit chance by chance to hit. A crit must pass the accuracy check twice. poedb's
  Blind page calls this the confirmation roll.
- Added critical strike multiplier is additive: +100% on the base gives 250%.

Keystones that change crits:

| Keystone | Effect | Ladder characters |
|---|---|---|
| Resolute Technique | Hits can't be evaded; never crit | 27 (8.3%) |
| Precise Technique | 40% more attack damage if accuracy is above maximum life; never crit | 17 (5.2%) |
| Elemental Overload | 40% more elemental damage after a crit in the past 8 seconds; crits deal no extra damage | 14 (4.3%) |
| Perfect Agony | Ailment DoT multiplier equals crit multiplier; crits deal no extra damage | 1 (0.3%) |

Counts are from our PoE1 corpus of 327 ladder characters.

## Accuracy and hit chance

Only **attacks** can miss. Spells always hit. PoB's chance to hit (`calcs.hitChance`), clamped 5% to 100%:

```
chance to hit = 125 × accuracy / (accuracy + (enemy evasion / 5) ^ 0.9)
```

| Accuracy source | Amount |
|---|---|
| Level | 2 per level above 1 (198 at level 100) |
| Dexterity | **+2 per point** |

A level-84 normal monster has 8,120 evasion. It takes about 2,000 accuracy for 90% hit chance, 2,400 for 95%
and 3,000 for 100%. PoB's boss settings scale enemy evasion, so read `HitChance` from `get_stats` instead of
guessing.

Enemy block also lowers PoB's `HitChance`. Blind on you gives 20% less accuracy.

## Resistance, exposure and penetration

PoB builds the enemy's resistance in this order:

1. **Base value** from `set_config`. The default pinnacle boss has **50%** elemental and **30%** chaos resistance.
2. **Curses and exposure** lower it. They add together. Exposure defaults to **‑10%**, and only the strongest
   exposure of each element counts.
3. **Cap and floor.** The result stays between ‑200% and the enemy's maximum (75% base, up to 90%).
4. **Penetration** is subtracted last, for hits only. PoB lets penetration take resistance below 0%.

| Tool | Hits | Damage over time and ailments |
|---|---|---|
| Curses and exposure | Yes | Yes |
| Scorch | Yes | Yes |
| Penetration | Yes | **No** |

For a damage-over-time or ailment build, penetration is wasted. Curses and exposure work for both.

## Speed

| Source | Effect in PoB |
|---|---|
| Attack speed | Weapon attacks per second × (1 + increased) × more |
| Cast speed | Gem cast time, scaled the same way |
| Dual wielding | 10% more attack speed |
| Frenzy charge (max 3) | 4% increased attack speed, 4% increased cast speed, 4% more damage each |
| Onslaught | 20% increased attack, cast and movement speed |
| Tailwind | 8% increased action speed |

Action speed changes everything the character does. Chill lowers it.

## Hits and damage over time

| Property | Hit | Damage over time |
|---|---|---|
| Can crit | Yes | No (ailments from crits get +50% DoT multiplier) |
| Penetration applies | Yes | No |
| Evaded, blocked, suppressed | Yes | No |
| Enemy armour applies | Yes (physical) | No; only enemy physical damage reduction |

PoB's formula for a damage-over-time instance:

```
DoT = base × (1 + increased) × more × (1 + DoT multiplier) × enemy resistance multiplier
```

- "Damage over time multiplier" is its own additive stat. It multiplies with increased and more.
- A skill's damage over time does not stack with itself unless the skill says so.
- PoB caps damage over time at **35,791,394 per second**, the game's integer limit.
- Ignite, bleed and poison are covered in [03-ailments.md](03-ailments.md).

## How PoB reports damage

| Field | What it holds |
|---|---|
| `AverageHit` | One hit after crits and enemy mitigation |
| `AverageDamage` | `AverageHit` × chance to hit |
| `TotalDPS` | `AverageDamage` × hits per second |
| `TotalDot` | The skill's own damage over time |
| `IgniteDPS`, `BleedDPS`, `TotalPoisonDPS` | Ailment damage per second |
| `ImpaleDPS` | Damage from impales |
| `TotalDotDPS` | Skill DoT + poison + ignite + bleed + burning and caustic ground + corrupting blood + decay |
| `CombinedDPS` | `TotalDPS` + `TotalDotDPS` + `ImpaleDPS` + mirages, × culling multiplier |
| `FullDPS` | `CombinedDPS` summed over socket groups marked "Include in Full DPS" |

- Culling strike kills at 10% life, so PoB multiplies `CombinedDPS` by 100 / 90 = **1.11**.
- `FullDPS` counts only the strongest ignite and bleed, and adds all poison and impale.
- Minion damage sits in the minion's own output (`Minion.CombinedDPS`), not in the player's `CombinedDPS`.
- All numbers use the current `set_config` enemy. Compare two setups only under the same config.

In our PoE1 corpus:

- 146 of the 299 ladder characters with player damage get 90% or more of `CombinedDPS` from hits. 79 get less
  than half from hits; their damage is mostly damage over time.
- The median `CombinedDPS` of those 299 is **3,065,358**.
- `FullDPS` is 0 for all 327 ladder characters, because no socket group is marked. It is above 0 for 100 of the
  140 guide loadouts.
- 28 of the 29 characters whose main skill is a minion skill show player `CombinedDPS` of 0.

## Advising

1. Read `CombinedDPS` with `get_stats` and state which enemy `set_config` assumes.
2. Look for missing "more" multipliers before adding "increased". Check supports with `list_valid_supports`.
3. For attacks, check `HitChance` before valuing crit. Every miss is also a lost crit.
4. Do not suggest penetration to a damage-over-time build. Suggest a curse or exposure.
5. For minion builds, read the minion's numbers.
6. Use `skill_info` for a skill's own conversion, base damage and damage effectiveness.

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/Modules/CalcOffence.lua` (`calcDamage`, `buildConversionTable`,
  `calcResistForType`, crit chance and multiplier, hit chance, penetration, DoT formula, `CombinedDPS`,
  `TotalDotDPS`, culling), `Modules/CalcDefence.lua` (`calcs.hitChance`), `Modules/CalcPerform.lua` (dexterity
  accuracy, Onslaught, Tailwind, Blind, exposure), `Modules/CalcSetup.lua` (base accuracy, crit multiplier, charge
  and dual wield mods), `Modules/Calcs.lua` (`calcFullDPS`), `Modules/Data.lua` (`data.misc`), `Data/Misc.lua`
  (`characterConstants`, `monsterEvasionTable`), `Modules/ConfigOptions.lua` (exposure, `enemyIsBoss`),
  `Modules/ModParser.lua` (culling strike), `TreeData/3_29/tree.json` (keystone text).
- Corpus: `S:\_projects_\_poe2_\corpus\poe1\index.json`, `stages.jsonl`, `xml/` (327 usable ladder characters,
  140 guide loadouts).
- https://poedb.tw/us/Blind
