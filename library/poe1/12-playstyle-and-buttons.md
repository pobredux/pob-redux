# Playstyle and buttons

How many keys a PoE1 build asks the player to press, how builds automate the rest, and how fast they move.
These shape which build a player enjoys, and no stat block shows them.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame. Counts are from the 327 ladder characters in our
PoE1 corpus unless a line says otherwise.

## How the app counts buttons

`build_summary` gives every skill a `press` class, from the skill types PoB calculates:

| `press` | Rule | Needs a key? |
|---|---|---|
| `active` | Everything not below | Yes |
| `persistent` | Reserves mana or life (auras, heralds, Arctic Armour), or a stance | No, turned on once |
| `trigger` | Triggered by a support or another skill | No |
| `granted` | Comes from an item | Only if the build uses it |

`activeSkills` also counts skills cast once per area: Raise Spectre, Animate Guardian, golems and the Righteous
Fire toggle. It counts a Vaal gem once, although its Vaal skill is a second key.

## What builds press

| `activeSkills` | Ladder characters (of 327) |
|---|---|
| 1–3 | 53 |
| 4 | 51 |
| 5 | 53 |
| 6 | **64** |
| 7 | 41 |
| 8 | 24 |
| 9–10 | 24 |
| 11 or more | 17 |

- **310 of 327** press 10 skills or fewer. The median is **6** (middle half 4 to 7).
- The same builds run a median 4 persistent skills and 2 triggered skills.
- Without the cast-once summons, the Righteous Fire toggle and Vaal skills, the median falls to **4 keys in
  combat**. 239 of 327 are at 5 or fewer, and 312 of 327 at 8 or fewer.
- Softcore characters press a median 5, hardcore and SSF 6. Deadeye has the lowest median (3) and Elementalist
  the highest (9).
- The 68 Maxroll endgame loadouts match: 6 active, 4 persistent, 3 triggered.

`sanity_check` reports "buttons" when a build has more than 10 active skills.

### What the keys are for

The skills most often classed `active`: Frostblink (161, plus 22 Frostblink of Wintry Blast), Shield Charge
(154), Flame Dash (104), Raise Spectre (98), Animate Guardian (65), Blood Rage (60), Leap Slam (54), Enduring Cry
(42).

- 316 of 327 press at least one movement skill; the median is 2.
- 158 press a buff such as Blood Rage.
- 114 cast a minion skill; 94 a curse or mark; 77 a Vaal skill; 54 a warcry.

Two of the median 4 combat keys are movement skills. That leaves about two keys for damage and buffs.

## Automation

238 of 327 characters have at least one triggered skill. The tools they use, with PoB's description and what the
ladder puts in them:

| Characters | Tool | What it does in PoB | Most common payload |
|---|---|---|---|
| **135** | Automation | Reserves mana; supported instant spells trigger over and over | Convocation 34, Withering Step 17, Brand Recall 10, Detonate Mines 10 |
| **118** | Cast when Damage Taken | Triggers spells when damage taken reaches a threshold | Vaal Molten Shell 27 |
| 84 | Mark On Hit | Supports mark curses; PoB counts the mark as triggered | Assassin's Mark 52, Sniper's Mark 27 |
| 42 | Autoexertion | Reserves mana; supported warcries trigger over and over | General's Cry 29, Seismic Cry 16, Intimidating Cry 16 |
| 30 | Arcanist Brand | A brand that triggers linked spells while attached | Blade Vortex 10, Elemental Weakness 9 |
| 19 | Spellslinger | Reserves mana; spells trigger from wand attack projectiles | |
| 11 | Manaforged Arrows | Bow attacks trigger after enough mana spent | |
| 10 | Cast On Critical Strike | An attack triggers a spell when it crits | |
| 10 | Cast while Channelling | A channelling skill triggers a spell | |

PoB's text says Cast when Damage Taken cannot trigger Vaal skills. The Vaal Molten Shell gem also grants plain
Molten Shell, which is the part that fires. Of 44 characters with Vaal Molten Shell, 35 trigger it.

Two patterns lower the key count:

1. **Reservation automation.** Automation and Autoexertion turn a pressed spell or warcry into a reserved one.
   They cost mana reservation, which is already at a median 90%. See [09-buildcraft.md](09-buildcraft.md).
2. **Condition triggers.** Cast when Damage Taken, Mark On Hit and Arcanist Brand need no reservation. They need a
   link and a skill that fits the trigger's rules.

Auras and heralds are the plain case: a median **4 reservation skills** per build, each turned on once.

## Movement skills and movement speed

318 of 327 characters have a movement skill; 218 have two.

| Characters | Movement skill |
|---|---|
| 165 | Frostblink (plus 22 Frostblink of Wintry Blast) |
| 154 | Shield Charge |
| 105 | Flame Dash |
| 54 | Leap Slam |
| 37 | Withering Step |
| 23 | Whirling Blades |

PoB's movement speed multiplier (`MovementSpeedMod`, with the flasks the export marks active):

| At least | Characters (of 327) |
|---|---|
| +15% | 317 |
| **+30%** | **302** |
| +50% | 280 |
| +75% | 228 |
| +100% | 161 |

The median is ×1.98. The sources: boots with a flat movement speed line (304 of 327), a Quicksilver Flask (238),
and 8% from helping Kraityn (15). `build_summary` reports `movementSpeedMod`. Movement speed on boots is covered
in [11-gear-and-flasks.md](11-gear-and-flasks.md).

## Flask automation

Instilling Orbs add a "Used when" enchantment to a flask, and the flask then fires on its own. PoB's data lists 16
options. Enchantments on the ladder's flasks:

| Flasks | Enchantment |
|---|---|
| **762** | Used when Charges reach full |
| 27 | Reused at the end of this Flask's effect |
| 17 | Used when you Hit a Rare or Unique Enemy, if not already in effect |
| 15 | Used when an adjacent Flask is used |

| Automated flasks | Characters (of 327) |
|---|---|
| 0 | 75 |
| 1–2 | 71 |
| 3 | 37 |
| 4 | **91** |
| 5 | 53 |

- 252 of 327 automate at least one flask; 144 automate four or more.
- None of the 187 life flasks carries a "Used when" enchantment. Life flasks stay on a key.
- **105 of 327** wear Mageblood or Foulborn Mageblood. PoB's text: the leftmost 2 to 4 magic utility flasks apply
  their effects constantly.
- 158 of 327 either automate all five flasks or wear Mageblood.

Maxroll's flask guide recommends "Used when Charges reach full" on utility flasks and prices it at 5 Instilling
Orbs and 5 Glassblower's Baubles per flask.

## What this means for advice

1. **Report keys, not gems.** "Six keys, four auras turned on once, two triggered guards" is accurate. "Twelve
   gems" is not.
2. **Aim for 10 or fewer active skills.** 310 of 327 ladder characters are there. Past 10 needs a reason.
3. **Add power as automation.** A reservation skill, a Cast when Damage Taken setup or Automation adds a skill
   without adding a key, if the mana reservation has room.
4. **Two movement skills and +30% movement speed** is the ladder norm.
5. **Automate utility flasks.** Keep the life flask on a key.
6. **Ask about button preference** when it is unclear. Some players want fewer keys and accept less damage.

## Sources

- PoB: `crates/pob-engine/lua/bridge.lua` (`build_summary` press classes), `Data/Global.lua` (skill types),
  `Data/Skills/act_int.lua`, `act_str.lua`, `sup_int.lua` and the other skill files (trigger gem descriptions),
  `Data/Gems.lua` (Vaal Molten Shell), `Data/EnchantmentFlask.lua`, `Data/Uniques/belt.lua` (Mageblood),
  `Modules/CalcSetup.lua` (Kraityn).
- Corpus: `corpus/poe1/index.json` and `xml/`, re-read through PoB 2.67.2 for each skill's calculated types and
  each flask's enchantment lines.
- Maxroll: https://maxroll.gg/poe/getting-started/flasks-for-beginners.
