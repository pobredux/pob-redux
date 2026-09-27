# Sustain and utility

Leech, regeneration, recoup, flasks, curses, marks, guard skills, movement skills and warcries. Numbers are PoB's
unless a line says otherwise.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame.

## Recovery at a glance

| Mechanic | What it recovers | PoB base value |
|---|---|---|
| Life leech | A share of hit damage dealt | Each instance **2% of maximum life per second**; total capped at **20% per second** |
| Mana leech | A share of hit damage dealt | Same as life: 2% per instance, **20%** cap |
| Energy shield leech | A share of hit damage dealt | 2% per instance, **10%** cap |
| Life regeneration | Modifiers only | **None** by default |
| Mana regeneration | Maximum mana | **1.75% per second** |
| Energy shield recharge | Maximum energy shield | **33.3% per second** after a **2 second** delay |
| Recoup | A share of damage taken from hits | Over **4 seconds** (3 with some modifiers) |

The constants are in [13-game-constants.md](13-game-constants.md). `get_stats` reads the results: `LifeRegen`,
`NetLifeRegen`, `LifeLeechRate`, `MaxLifeLeechRate`, `LifeRecoup`, `ManaRegen` and `EnergyShieldRecharge`.

## Leech

- One leech instance holds at most **10% of the maximum pool**. A larger instance lasts longer, because each one
  recovers at 2% per second.
- All instances together stop at the cap in the table above.
- **Instant leech** skips the cap. The Leech Mastery "5% of Leech is Instant" is the common source (76 of 327
  ladder characters in our PoE1 corpus).

Keystones that change leech:

| Keystone | Effect on recovery | Ladder characters |
|---|---|---|
| Ghost Reaver | Leeches energy shield instead of life; the energy shield leech cap doubles; energy shield cannot recharge | 26 of 327 |
| Eternal Youth | 50% less maximum life leech rate and 50% less life regeneration; energy shield recharge applies to life | 18 |
| Vaal Pact | Life leech from melee damage is instant; no other life recovery | 1 |

The rest of the keystones are in [14-keystones.md](14-keystones.md).

## Regeneration

- Life regeneration comes only from modifiers and auras such as Vitality (69 of 327 ladder characters).
- **Zealot's Oath** applies life regeneration to energy shield instead (36 of 327).
- `NetLifeRegen` is regeneration after degeneration. Read it for a build that costs or burns life.

## Recoup

Recoup returns a share of the damage taken from hits over **4 seconds**, or 3 with some modifiers. PoB also
handles recoup by damage type and recoup to mana or energy shield.

## Flasks

A character has **5 flask slots**. Tinctures also go in flask slots, and only one tincture can be active at a
time. How flasks fit the rest of the gear is in [11-gear-and-flasks.md](11-gear-and-flasks.md).

### Charges

Killing monsters fills flasks. Maxroll gives the base charges per kill: normal **1**, magic **3.5**, rare **6**,
unique **11**. Each flask has its own maximum and its own cost per use.

### Life flasks

| Flask | Recovers | Over | Charges per use / maximum | Level |
|---|---|---|---|---|
| Divine Life Flask | 2,400 life | 3.5 s | 15 / 45 | 60 |
| Eternal Life Flask | 2,080 life | 2 s | 15 / 45 | 65 |

### Utility flasks

| Flask | Effect in PoB | Duration | Charges per use / maximum |
|---|---|---|---|
| Quicksilver | 40% increased movement speed | 6 s | 30 / 60 |
| Granite, Jade | +1,500 armour; +1,500 evasion rating | 6 s | 30 / 60 |
| Basalt, Stibnite | 20% more armour; 20% more evasion rating | 8 s | 40 / 60 |
| Silver | Onslaught | 6 s | 40 / 60 |
| Diamond | 100% increased global critical strike chance | 6 s | 20 / 40 |
| Ruby, Sapphire, Topaz | +40% to one resistance, +5% to its maximum | 8 s | 20 / 50 |
| Amethyst | +35% chaos resistance | 6.5 s | 35 / 65 |
| Quartz | +10% chance to suppress spell damage, Phasing | 6 s | 30 / 60 |

### "Used when" enchantments

An Instilling Orb makes a flask use itself. PoB and Maxroll list the same 16 conditions:

- Used when Charges reach full; Reused at the end of this Flask's effect; Used when an adjacent Flask is used.
- Used when you Use a Guard Skill; Used when you lose a Guard Skill Buff; Used when you Use a Travel Skill.
- Used when you Hit a Rare or Unique Enemy, if not already in effect; Used when you Block; Used when you take a
  Savage Hit (15% of maximum life); Used when you use a Life Flask.
- Used when you become Frozen, Chilled, Shocked, Ignited or Poisoned; Used when you start Bleeding.

An Enkindling Orb adds one of five effects instead: more flask effect, longer duration, faster charge recovery,
fewer charges per use or more maximum charges. Each comes with "Gains no Charges during Effect".

### How PoB applies flasks

- PoB applies a flask only while its slot is toggled active. `build_summary` reports `flasksEquipped` and
  `flasksActive`.
- Two utility flasks with the same base do not stack. PoB keeps the larger value of each line.

### What the corpus uses

- 317 of 327 ladder characters fill all five slots.
- 762 of 1,610 flasks carry "Used when Charges reach full".
- 171 of 327 carry a life or hybrid flask. 23 carry a mana or hybrid flask.
- Quicksilver is the most common base (241 flasks). 51 characters use a tincture.
- The most common unique flasks are Cinderswallow Urn (41), Rumi's Concoction (30), Wine of the Prophet (30),
  Progenesis (24) and Atziri's Promise (21).

## Curses and marks

- The base limit is **1 curse** on an enemy (`EnemyCurseLimit`).
- In PoE1, **marks are curses**. A mark takes one of the curse slots, and only one mark can be active at a time.
  poedb's mark text says the same. With a limit of 1, a mark and a hex compete.
- The limit rises by 1 from the notable **Whispers of Doom**, the Occultist's **Unholy Authority** (which also lets
  hexes affect hexproof enemies), the Ascendant's Occultist option, and a few uniques.
- **Hex Master** makes hexes last forever at 20% less curse effect.
- **Doom.** PoB has a "Doom on Hex" config (`multiplierHexDoom`). It only appears when a hex has a maximum doom, and
  no gem in PoB's 3.29 data has one by itself.

Ways to apply curses without casting them:

| Method | Ladder characters |
|---|---|
| Mark On Hit (a support for marks) | 84 of 327 |
| Arcanist Brand (triggers linked spells) | 30 |
| Hexpass, Cursed Ground, Blasphemy, Eldritch Blasphemy, Hextouch | 2 to 4 each |

In our PoE1 corpus, 273 of 327 ladder characters enable a curse or mark gem, and 222 enable exactly one. The most
common are Assassin's Mark (83), Elemental Weakness (47), Sniper's Mark (44), Despair (44), Punishment (38) and
Vulnerability (34). Whispers of Doom is allocated on 16 ladder trees and anointed on 10 amulets.

## Guard skills

Guard skills share one cooldown. PoB applies **one non-Vaal guard buff at a time**, and Vaal Molten Shell replaces
all of them.

| Skill | What it does (gem level 20) | Duration | Cooldown |
|---|---|---|---|
| Steelskin | Takes 70% of hit damage up to a limit; immunity to bleeding | 1.5 s | 3 s |
| Molten Shell | Takes 75% of hit damage up to 10% of armour (at most 5,000); adds armour; reflects fire damage | 3 s | 4 s |
| Immortal Call | 35% less physical and 34% less elemental damage taken; each endurance charge spent (up to 5) adds 20% duration and 15% less physical damage | 1 s | 3 s |
| Arcane Cloak | Spends mana to take 75% of hit damage | 3 s | 4 s |
| Vaal Molten Shell | Takes 39% of hit damage up to 20% of armour (at most 10,000); reflects fire damage each second | 9 s | 50 souls |

- Builds usually trigger them with **Cast when Damage Taken**. That support cannot trigger Vaal skills,
  channelling skills or skills with a reservation.
- In our PoE1 corpus, 227 of 327 ladder characters socket a guard skill: Steelskin 66, Molten Shell 63, Immortal
  Call 46, Vaal Molten Shell 44, Arcane Cloak 10. 118 socket Cast when Damage Taken.
- **Every non-Vaal guard gem in these poe.ninja exports is disabled** (186 of 186), so PoB leaves the buff out.
  Enable the gem before judging an imported build's defences.

## Movement skills

319 of 327 ladder characters socket a travel skill.

| Skill | Ladder characters | Notes from PoB's data |
|---|---|---|
| Frostblink | 165 (plus 22 on Frostblink of Wintry Blast) | Blink skill; 2.6 s cooldown |
| Shield Charge | 154 | No cooldown; uses off-hand damage |
| Flame Dash | 105 | Blink skill; 3 stored uses, 3.5 s cooldown |
| Leap Slam | 54 | No cooldown; axe, mace, sceptre, sword or staff |
| Withering Step | 37 | Blink skill; Elusive and Phasing; 3 s cooldown |
| Whirling Blades | 23 | No cooldown; dagger, claw or one-handed sword |

Blink skills share a cooldown. Movement speed is covered in
[12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

## Warcries

A warcry taunts nearby enemies and gives a buff. Most also **exert** the next attacks. At gem level 20 most have an
**8 second cooldown**; General's Cry has 3 seconds.

- PoB scales a warcry's buff by its uptime: buff duration ÷ cooldown, at most 100%.
- **Autoexertion** reserves mana to trigger its linked warcries over and over.
- In our PoE1 corpus, 73 of 327 ladder characters socket a warcry: Enduring Cry 43, Autoexertion 43, Urgent Orders
  36 (a warcry support), General's Cry 29, Battlemage's Cry 24, Rallying Cry 24.

## Advising on sustain

1. **Match the mechanic to the problem.** A character that dies to one hit needs mitigation or a larger pool. Leech
   and regeneration recover over time.
2. **Read the leech cap.** `LifeLeechRate` at `MaxLifeLeechRate` means more leech adds nothing; instant leech or a
   higher cap does.
3. **Check the flasks are active** and that no two share a base.
4. **Count curses against the limit.** A mark uses a curse slot.
5. **Enable guard gems** on an imported build before judging defences.

## Sources

- PoB PoE1 (`src-tauri/resources/pob1`): `Data/Misc.lua` (`characterConstants` leech and recovery values),
  `Modules/Data.lua` (`LeechRateBase`), `Modules/CalcOffence.lua` (leech instances, instant leech, rate cap),
  `Modules/CalcDefence.lua` (leech caps, recoup, regeneration), `Modules/CalcPerform.lua` (flasks, curse slots,
  guard slots, warcry uptime, doom), `Modules/CalcSetup.lua` (curse limit, tincture limit),
  `Modules/ConfigOptions.lua` (`multiplierHexDoom`), `Data/Bases/flask.lua`, `Data/EnchantmentFlask.lua`,
  `Data/Skills/act_str.lua`, `act_int.lua` and the other skill files (guard, movement, warcry data),
  `Classes/ItemsTab.lua` (flask slots, tinctures), `TreeData/3_29/tree.lua` (keystone and notable text).
- Our PoE1 corpus: `corpus/poe1/index.json`, `stages.jsonl`, `tree-nodes.json` and `xml/*.xml` (flasks, curses,
  guard, movement and warcry gems on 327 ladder characters).
- Maxroll: <https://maxroll.gg/poe/resources/flasks>,
  <https://maxroll.gg/poe/getting-started/defenses-for-beginners>
- poedb: <https://poedb.tw/us/Assassins_Mark>
