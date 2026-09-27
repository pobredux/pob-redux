# Gear and flasks

What each PoE1 gear slot carries, how many sockets and links it takes, and what the five flask slots hold on
real characters.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame. Counts are from the 327 ladder characters in our
PoE1 corpus unless a line says otherwise.

## What each slot does

| Slot | Main job | Sockets (PoB limit) | Unique on the ladder |
|---|---|---|---|
| Weapon | Damage base for attacks, spell levels and damage for casters | 3 one-handed, 6 two-handed | 143 of 327 |
| Off-hand | Shield (214 of 327), quiver (12) or a second weapon | 3 | 132 of 257 |
| Body armour | The 6-link, the largest defence base | 6 | 127 |
| Helmet | Life, resistances, a 4-link | 4 | 129 |
| Gloves | Life, resistances, a 4-link | 4 | 63 |
| Boots | **Movement speed**, life, resistances | 4 | 77 |
| Rings | Life, resistances, chaos resistance | none | 142 and 162 |
| Amulet | Life, skill levels, crit multiplier | none | 185 |
| Belt | Life and resistances, or a build-defining unique | none | **238** |

The off-hand row counts the 257 characters with something in that slot; it is empty for the other 70. Gloves and
boots stay rare the most often: 264 and 250 of 327.

## Weapons

Main-hand types on the ladder: wand 100, sceptre 58, one-handed sword 44, two-handed axe 30, staff 27, claw 19,
bow 12, dagger 11. Mods on 172 rare weapons:

| Rares | Mod |
|---|---|
| 92 | % increased Attack Speed |
| 62 | % increased Critical Strike Chance |
| 59 | % increased Spell Damage |
| 58 | % to Global Critical Strike Multiplier |
| 47 | % increased Physical Damage |
| 42 | + to Level of all Spell Skill Gems |

## Body armour and links

- **278 of 327** body armours are 6-linked; 306 have six sockets.
- 20 have no sockets. 18 of them are Kaom's Heart, which PoB lists as "Has no Sockets" with +1,000 life. Those
  characters put the main skill in the helmet (9), gloves (4), boots (4) or off-hand (1).
- 48 main-hand weapons are 6-linked. Only two-handed weapons can hold six sockets.
- 291 of 327 characters have at least one 6-linked item.
- 4-links: helmet 264, gloves 230, boots 221.

On 200 rare body armours, the most common mods are Stun and Block Recovery (123) and maximum energy shield (121).
Only 53 carry flat life, so most life comes from the other slots.

## Boots

**304 of 327** boots have a flat "% increased Movement Speed" line. The median is 25%, and 30% is the most common
value (72 characters). 23 do not; 9 of those wear Bubonic Trail.

PoB's boots prefix tiers need item level 1 (10%), 15 (15%), 30 (20%), 40 (25%), 55 (30%) and 86 (35%). On 250
rare boots: movement speed 241, life 197, chaos resistance 110.

## Rings, amulet and belt

These slots have no sockets. They carry most of the life and resistance a build is short of.

| Slot | Rares | Life | A fire, cold or lightning line | Chaos resistance |
|---|---|---|---|---|
| Ring (first) | 185 | 150 | 232 of 327 (any rarity) | 90 |
| Amulet | 142 | 96 | 157 of 327 | |
| Belt | 88 | 76 | 201 of 327 | 28 |

Amulets also carry skill gem levels (+1 to all skill gems on 37 rares) and crit multiplier (46). 52 of 88 rare
belts have an abyssal socket.

Resistance lines are spread over every armour slot: boots 245, gloves 242, first ring 232, second ring 217,
body 206, belt 201, helmet 199, amulet 157. When a build is short, name the slot that has a free suffix.

## Influence and eldritch implicits

PoB reads eight item influences and calculates their mods: Shaper, Elder, and the Crusader, Redeemer, Hunter and
Warlord conquerors, plus Searing Exarch and Eater of Worlds. The eldritch implicits in PoB's data
(`ModEldritch.lua`) exist for helmet, body armour, gloves, boots and amulet.

- **173 of 327** characters wear at least one Shaper, Elder or conqueror item, most often in the off-hand (54).
- **299 of 327** wear at least one eldritch implicit: gloves 252, boots 223, body 176, helmet 164.
- Most common eldritch implicits: increased Action Speed (104 items), physical damage taken as fire (82), mana
  cost efficiency (75).

## Flasks

A character has **5 flask slots**. PoB and poedb agree. PoB's bases come in four kinds: life, mana, hybrid and
utility. Life and mana flasks refill. Utility flasks grant a buff for a few seconds; PoB applies it while the flask
is marked active.

317 of 327 ladder characters fill all five. Flask bases, by characters carrying at least one:

| Characters | Base | Effect in PoB |
|---|---|---|
| **238** | Quicksilver Flask | 40% increased movement speed |
| 171 | Silver Flask | Onslaught |
| 146 | Divine Life Flask | 2,400 life (level 60) |
| 137 | Ruby Flask | +40% fire resistance, +5% maximum |
| 120 | Granite Flask | +1,500 armour |
| 85 | Quartz Flask | Phasing, +10% spell suppression |
| 82 | Topaz Flask | +40% lightning resistance, +5% maximum |
| 78 | Diamond Flask, Sapphire Flask | |
| 67 | Gold Flask | 30% increased item rarity |

**Life flasks.** 156 of 327 run no life flask. 62 of those are Chaos Inoculation, and 79 wear Mageblood (some
are both). 148 run exactly one life flask. A life build without Mageblood should keep one.

**Unique flasks.** 263 of 1,610 flasks are unique: Cinderswallow Urn 41, Wine of the Prophet 30, Rumi's
Concoction 30, Progenesis 24, Atziri's Promise 21.

**Mods.** Most of the 1,321 magic flasks trade duration for effect ("% increased effect" 539, "% reduced
Duration" 491). Flask lines that answer an ailment or curse, by characters:

| Characters | Answer |
|---|---|
| 177 | Reduced effect of curses during the flask |
| 164 | Immunity to bleeding (145 life flasks carry it) |
| 76 | Ignite or burning |
| 67 | Shock |
| 40 | Freeze or chill |

**Tinctures** also sit in flask slots. 51 of 327 carry one: Prismatic 34, Rosethorn 13, Blood Sap 10.

**Enchantments.** 252 of 327 characters have at least one flask with an Instilling "Used when" enchantment, and
462 flasks carry the Enkindling "Gains no Charges during Effect" line. See
[12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

## Uniques most equipped

| Slot | Most equipped (of 327) |
|---|---|
| Belt | **Mageblood 82** (+23 Foulborn Mageblood), Headhunter 39, Darkness Enthroned 18 |
| Rings | Kalandra's Touch 40, The Taming 26, Nimis 24, Death Rush 20 |
| Boots | Ralakesh's Impatience 29, Bubonic Trail 9, Replica Alberon's Warpath 9 |
| Off-hand | Aegis Aurora 19, Svalinn 12, Dawnbreaker 11 |
| Body armour | Kaom's Heart 18, Cloak of Flame 10, Doppelganger Guise 9 |
| Amulet | Yoke of Suffering 18, Replica Dragonfang's Flight 18, Ashes of the Stars 17 |
| Weapon | The Dark Seer 17, Nycta's Lantern 11, Ephemeral Edge 10 |
| Helmet | Heatshiver 17, The Dark Monarch 9 |
| Gloves | No unique above 6 characters |

Mageblood makes the leftmost 2 to 4 magic utility flasks apply their effects constantly, in PoB's text. Kalandra's
Touch reflects the opposite ring. Read a unique's text with the app before swapping it for a rare.

## Advising on gear

1. **Fix links first.** A main skill outside a 5- or 6-link is the largest single gap.
2. **Boots need movement speed.** 304 of 327 have it.
3. **Resistances come from every slot.** Name the slot with a free suffix. `optimise_gear` and `craft_rare` find
   one.
4. **Five flasks, one of them a Quicksilver,** and a life flask with bleed immunity for life builds.
5. **Use `list_bases` and `list_affixes`** to see what a base and slot can roll before suggesting a craft.

## Sources

- PoB: `Data/Bases/*.lua` (socket limits, flask bases and buffs), `Data/ModExplicit.lua` (boots movement speed),
  `Data/ModEldritch.lua`, `Modules/ItemTools.lua` (influence list), `Data/Uniques/belt.lua`, `body.lua`,
  `ring.lua`, `Data/EnchantmentFlask.lua`.
- Corpus: `corpus/poe1/xml/` item text for every ladder character, re-read through PoB 2.67.2 (slot, rarity,
  sockets, influences, implicit, explicit and enchantment lines).
- poedb: https://poedb.tw/us/Flasks (5 flask slots, flask level requirements).
