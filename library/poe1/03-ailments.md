# Ailments

Written against 3.29 (PoB 2.67.2, tree 3_29). Numbers come from PoE1 PoB (`data.misc`, `data.nonDamagingAilment`
and `CalcOffence.lua`) unless a line names another source. The app calculates with PoB.

## The two kinds

- **Damaging ailments** deal damage over time: ignite, bleed, poison.
- **Non-damaging ailments** apply a debuff: chill, freeze, shock, and the alternative ailments scorch, brittle
  and sap.

Scorch, brittle and sap come only from sources that name them, such as the keystone Secrets of Suffering. A
critical strike does not guarantee them.

## How a hit inflicts an ailment

| Ailment | Needs | On a critical strike |
|---|---|---|
| Chill | Any cold damage; **every cold hit chills** | Always |
| Freeze | Chance to freeze and cold damage | **Always** |
| Shock | Chance to shock and lightning damage | **Always** |
| Ignite | Chance to ignite and fire damage | **Always** |
| Bleed | Chance to bleed, physical damage, **attacks only** | Only with chance |
| Poison | Chance to poison, physical or chaos damage | Only with chance |
| Scorch, brittle, sap | A source that grants them | Only with chance |

- Enemy avoidance or immunity lowers the chance to 0.
- Elemental Overload and Perfect Agony change the crit rules. `set_config` `ailmentMode` switches PoB between
  "average" and "crits only".
- Modifiers such as "physical damage can ignite" let other types count.

## Damaging ailments

| Ailment | Source damage | Damage per second | Base duration | Instances that deal damage |
|---|---|---|---|---|
| Ignite | Fire | **90%** | **4 s** | 1 (the strongest) |
| Bleed | Physical | **70%** | **5 s** | 1 (the strongest) |
| Poison | Physical + chaos | **30%** | **2 s** | All of them |

- **Totals over the base duration:** ignite 360% of the fire in the hit, bleed 350%, one poison 60%.
- **Bleed against a moving enemy deals 200% more damage**, three times the normal rate.
- **Ailments from a critical strike get +50% damage over time multiplier.**
- PoB rebuilds the ailment from the hit's damage using only modifiers that can apply to that ailment: damage over
  time, the ailment itself, its damage type and generic damage.
- Penetration does not apply to ailments. Curses, exposure and shock do.
- "Faster" modifiers (burn faster, bleed faster, poison faster) shorten the duration and raise damage per second.
  Total damage stays the same.
- With several ailments of one kind on the target, PoB averages which roll is the strongest (`IgniteRollAverage`,
  `BleedRollAverage`).

Keystones that change the stacking rules:

| Keystone | Effect |
|---|---|
| Crimson Dance | Bleed up to 8 times; 50% less bleed damage; no bonus against moving enemies |
| Voracious Flame | One more ignite; base ignite duration 1 second; 25% less ignite damage; fire damage only |

## Non-damaging ailments

| Ailment | Effect on the target | Default | Min | Max | Duration |
|---|---|---|---|---|---|
| Chill | Reduced action speed | 10% | 5% | **30%** | 2 s |
| Shock | Increased damage taken | 15% | 5% | **50%** | 2 s |
| Freeze | Cannot act | | 0.3 s | **3 s** | the effect is the duration |
| Scorch | Lower elemental resistances | 10% | | **30%** | 4 s |
| Brittle | Flat critical strike chance against it | 2% | | **6%** | 4 s |
| Sap | Less damage dealt | 6% | | **20%** | 4 s |

"Default" is the value from a source that is not a hit, such as chilled ground.

From a hit, the effect depends on the damage of the matching type against the target's **ailment threshold**:

```
chill, shock, scorch:  50 × (hit / threshold) ^ 0.4
brittle:               10 × (hit / threshold) ^ 0.4
sap:                   33.3 × (hit / threshold) ^ 0.4
```

| To reach | Hit needed, as % of threshold |
|---|---|
| 15% shock | 4.9% |
| **50% shock** (cap) | **100%** |
| 30% chill, 30% scorch, 6% brittle, 20% sap (caps) | 27.9% |
| Minimum freeze (0.3 s) | 5% |

- Increased effect of shock or chill multiplies the result before the cap.
- One shock and one scorch at a time, unless a modifier lets them stack.
- A level-84 normal monster has an ailment threshold of **16,265**, equal to its life. Boss settings in
  `set_config` raise it far higher. PoB's shock and chill breakdowns show the threshold it used.

## Ailments on you

- A shock on you defaults to **15%** in PoB's config, up to **50%**. A chill defaults to **10%**, up to **30%**.
- "Chance to avoid" at 100% equals immunity. PoB adds avoid-all-ailments and avoid-elemental-ailments to each type.
- Poison and other chaos damage over time bypass energy shield.

### Sources of avoidance and immunity

| Source | Effect |
|---|---|
| Captain Tanner Lightfoot (Brine King upgrade) | 100% chance to avoid being frozen |
| Shock and Horror (Brine King upgrade) | 50% reduced effect of chill on you |
| Soul of Abberath | 60% less ignite duration on you |
| Soul of Shakari | 50% less poison duration on you; cannot be poisoned while at least 3 poisons are on you |
| Soul of Ralakesh | No extra bleed damage while moving; 25% reduced physical damage over time taken while moving |
| Soul of Garukhan | 60% reduced effect of shock on you |
| Charge Mastery | Cannot be ignited, chilled or shocked at maximum endurance, frenzy or power charges |
| Armour and Evasion Mastery | Immune to bleeding (helmet armour above evasion) or poison (the reverse) |
| Crystal Skin, Elegant Form | 20% chance to avoid elemental ailments |
| Thick Skin | 10% increased life, 8% chance to avoid elemental ailments |
| Stormshroud (jewel) | Chance to avoid shock applies to all elemental ailments |
| Ancestral Vision (jewel) | Spell suppression also applies to elemental ailment avoidance at 50% |
| Frigid Wake (Occultist) | Cannot be chilled or frozen |
| Pyromaniac (Saboteur) | Immune to ignite and shock |

PoB applies every soul of the chosen god, so a pantheon choice in `set_config` counts as fully upgraded. Use
`node_info` for mastery effects and `alloc_node` with `effect` to pick one.

### Flask suffixes

| Ailment | "If used while affected" (seconds of immunity) | "During effect" (less flask duration) |
|---|---|---|
| Bleed and corrupted blood | Sealing 6–8, Alleviation 9–11, Allaying 12–14, Assuaging 15–17 | Lizard, Skink, Iguana |
| Chill and freeze | Convection 6–8, Thermodynamics 9–11, Entropy 12–14, Thawing 15–17 | Deer, Walrus, Penguin |
| Ignite | Damping 6–8, Quashing 9–11, Quelling 12–14, Quenching 15–17 | Urchin, Mussel, Starfish |
| Shock | Earthing 6–8, Grounding 9–11, Insulation 12–14, the Dielectric 15–17 | Conger, Moray, Eel |
| Poison | Antitoxin 6–8, Remedy 9–11, Cure 12–14, Antidote 15–17 | Skunk, Hedgehog, Opossum |

"During effect" versions cost 35% to 49% less flask duration. Use `list_affixes` on a flask base to see tiers.

## What ladder builds use

In our PoE1 corpus of 327 ladder characters:

| Ailment | Most common answer | Characters |
|---|---|---|
| Bleed | A flask with bleed immunity | 164 (50.2%) |
| Freeze and chill | The Brine King | 152 (46.5%) |
| Ignite | Soul of Abberath | 100 (30.6%) |
| Bleed while moving | Soul of Ralakesh | 76 (23.2%) |
| Chill | Chill avoidance or immunity on gear (not flasks) | 75 (22.9%) |
| Shock | A shock flask, gear or mastery | 75 (22.9%) |
| All elemental | Thick Skin | 41 (12.5%) |
| All elemental | Stormshroud / Ancestral Vision | 38 (11.6%) / 28 (8.6%) |
| Poison | A flask, gear or mastery | 31 (9.5%) |

Counts use equipped items and chosen mastery effects only.

Other major gods: Lunaris 96, Solaris 39, Arakaali 38. Other minor gods: Tukohama 46, Shakari 28, Garukhan 26,
Ryslatha 24.

## Advising

1. For an ailment build, get chance to inflict to 100% first. Check it with `get_stats`.
2. For shock and chill, compare the hit with the target's threshold. A small hit on a boss applies a small shock.
3. For defence, check the bleed flask and the pantheon in `build_summary` before suggesting passives.
4. Set the pantheon with `set_config`, then re-read the numbers.

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/Modules/Data.lua` (`data.misc` bleed, poison and ignite values,
  `data.nonDamagingAilment`), `Modules/CalcOffence.lua` (ailment chance, `calcAilmentDamage`,
  `calcAverageSourceDamage`, bleed, poison and ignite sections, non-damaging effect formulas, freeze breakdown,
  enemy ailment threshold), `Modules/CalcPerform.lua` (ailment effects on enemies and on you, avoidance),
  `Modules/CalcSetup.lua` (moving bleed bonus, crit DoT multiplier, pantheon), `Modules/PantheonTools.lua`,
  `Data/Pantheons.lua`, `Data/ModFlask.lua`, `Data/Uniques/jewel.lua` (Stormshroud, Ancestral Vision),
  `Data/Misc.lua` (`monsterAilmentThresholdTable`, `characterConstants`), `TreeData/3_29/tree.json` (notables,
  masteries, ascendancy nodes, keystones).
- Corpus: `S:\_projects_\_poe2_\corpus\poe1\index.json`, `stages.jsonl`, `xml/` (327 usable ladder characters;
  pantheon from each build, item text and mastery effects).
- https://poedb.tw/us/Bleeding
- https://poedb.tw/us/Poison
- https://poedb.tw/us/Ignite
