# Buildcraft

What the builds in our PoE1 corpus do: their main skills, supports, keystones, auras, defences and headline
numbers. Use it to check whether a build looks like the ones that work.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame.

## What was counted

- **327 ladder characters** from poe.ninja: the top of each ascendancy, 8 softcore (SC), 4 hardcore (HC) and 4
  solo self-found (SSF). 323 of them are level 100.
- **136 loadouts from 27 Maxroll guides**, loaded through the same PoB.

The ladder sample takes about 16 characters from every ascendancy. It shows what top characters do. It does not
show which ascendancy or skill is popular. For popularity, this file quotes poe.ninja's own Allflame counts.

The main skill is the skill in the build's largest link, leaving out auras, movement skills and guard skills.
Ties go to the higher PoB DPS. **47 of 327** ladder characters show under 50,000 DPS in PoB:

- 14 are Luminary. Their damage comes from a hired Mercenary, and PoB's calculation modules do not model it.
- 27 of the other 33 reserve 85% or more of their mana or life. That is the shape of an aura or curse support
  character.

The damage tables below use the other 280.

## Ascendancies people play

poe.ninja lists characters from level 80. Allflame counts at the time of writing:

| Ascendancy | SC (124,408) | HC (19,972) | SSF (36,636) |
|---|---|---|---|
| Luminary | **29,080** | 3,753 | 9,087 |
| Elementalist | 17,625 | 2,874 | 7,621 |
| Occultist | 11,541 | 1,145 | 2,077 |
| Necromancer | 9,191 | 1,308 | 1,628 |
| Chieftain | 8,380 | 1,892 | 2,016 |

The least played in SC are Warden (366), Saboteur (536) and Trickster (968). Few players run them, so there is
less evidence about what works for those three.

## Main skills

Top main skills in our PoE1 corpus, 280 ladder characters with real DPS (transfigured versions grouped under the
base skill):

| Characters | Main skill |
|---|---|
| 20 | Kinetic Blast |
| 13 | Earthshatter |
| 12 | Blade Blast, Righteous Fire |
| 10 | Winter Orb, Penance Brand |
| 8 | Smite, Ethereal Knives |

The 280 characters use **80 different main skills**. poe.ninja's SC main-skill counts put Winter Orb (9,275),
Righteous Fire (5,803), Animate Guardian (5,656), Raise Spectre (4,960) and Ethereal Knives (3,763) on top.

Other shares among the 280:

- **Transfigured gem** as the main skill: 122 of 280.
- **Spells** 146, **attacks** 133.
- **Vaal gem** as the main skill: 33. **Minion** main skill: 21.
- Totems 11, brands 10, mines 6, traps 5.

## Supports on the main skill

| Supports on the main skill | Ladder characters (of 324 with a main link) |
|---|---|
| **5 (a 6-link)** | **246** |
| 4 | 15 |
| 3 | 31 |
| 0–2 | 32 |

Among the 280 damage characters, **237 of 279** run 5 supports. The main link sits in the body armour for 233 of
279, the weapon for 22 and the helmet for 17. Half of the 18 Kaom's Heart wearers (a body armour with no sockets)
put the main skill in the helmet.

Most common supports in the main link (damage characters):

| Characters | Support |
|---|---|
| 85 | Increased Critical Damage |
| 48 | Inspiration |
| 46 | Volatility |
| 45 | Greater Multistrike |
| 41 | Elemental Damage with Attacks |

Call `list_valid_supports` for the legal options on a skill, and `skill_info` for what a support does. The
`sanity_check` review flags a main skill with under 4 supports at level 68, or under 5 at level 85.

## Keystones

From `build_summary` `keystones`, which includes keystones granted by jewels. Texts are PoB's.

| Ladder characters (of 327) | Keystone | Effect in one clause |
|---|---|---|
| **67** | Chaos Inoculation | Maximum life becomes 1; immune to chaos damage |
| 45 | Unwavering Stance | Cannot evade attacks; cannot be stunned |
| 45 | Versatile Combatant | Spell block from overcapped attack block |
| 42 | Supreme Ostentation | Ignore attribute requirements (Elegant Hubris jewel) |
| 39 | Ghost Dance | Ghost Shrouds recover energy shield when hit |
| 36 | Eldritch Battery | Energy shield protects mana instead of life |
| 36 | Zealot's Oath | Life regeneration applies to energy shield |
| 35 | Resolute Technique | Hits cannot be evaded; never crit |
| 29 | Blood Magic | Skills cost and reserve life; no mana |
| 27 | Mind Over Matter | 40% of damage taken from mana first |

Inner Conviction (16), Tempered by War (16) and Corrupted Soul (13) also come from timeless jewels. See
[14-keystones.md](14-keystones.md) for every keystone and its downside.

## Auras, heralds and reservation

A PoE1 build turns its reservation skills on once and leaves them on. Counted from the skills' own PoB types
(item-granted skills left out):

| Characters (of 327) | Reservation skill |
|---|---|
| **135** | Automation |
| 115 | Flesh and Stone |
| 95 | Precision, Tempest Shield |
| 69 | Vitality |
| 57 | Herald of Purity |
| 55 | Purity of Elements, Arctic Armour |
| 53 | Determination |

- A build runs a median of **4 reservation skills** (middle half 3 to 5).
- Reserving auras per build: 0 for 22, 1 for 77, 2 for 72, 3 for 82, 4 or more for 74.
- Heralds: 233 of 327 run none; 63 run one.
- Enlighten appears in 149 of 327 builds.

**Mana reserved.** The 298 characters without Blood Magic reserve a median **90% of their mana** (middle half 85%
to 96%). 266 of 298 reserve 80% or more. The median leaves 92 mana unreserved. The 29 Blood Magic characters
reserve life instead; 13 of them are Luminary.

`build_summary` reports `manaReserved`, `manaUnreserved` and `lifeReserved`. `sanity_check` flags a main skill
that costs more than the unreserved pool.

## Life or energy shield

| Defence | Ladder characters (of 327) | Median life | Median ES |
|---|---|---|---|
| Life | **192** | 5,182 | 183 |
| Life with ES at half of life or more | 34 | 4,151 | 2,904 |
| Chaos Inoculation | **67** | 1 | 9,009 |
| Low life (50%+ of life reserved) | 24 | 2,233 | 970 |
| ES above life, no CI | 10 | 3,717 | 4,668 |

Chaos Inoculation is most common in SC: 41 of 160 SC, 18 of 83 HC and 8 of 84 SSF characters.

## Typical numbers by league type

Life counts life and hybrid characters. DPS counts the characters with real DPS. Ranges are the middle half.

| League | Life | CI energy shield | EHP | DPS |
|---|---|---|---|---|
| SC | 4,289 (3,330–5,951) | 7,954 | 76,771 | 4.9 million (1.3–23 million) |
| HC | **5,959** (5,143–6,947) | 13,479 | **139,447** | 3.5 million (1.2–9.1 million) |
| SSF | 5,033 (4,453–5,826) | 10,190 | 82,908 | 3.0 million (1.1–8.1 million) |

HC characters carry about 1,700 more life and nearly twice the EHP of SC characters. Their median physical
maximum hit is 25,348 against 11,314 in SC. Elemental resistances are at 75% or more for 108 of 160 SC, 76 of 83
HC and 82 of 84 SSF characters.

## Ladder against Maxroll endgame loadouts

68 Maxroll loadouts labelled endgame or aspirational, from 26 guides:

| Measure | Ladder (327) | Maxroll endgame (68) |
|---|---|---|
| Main skill with 5 supports | 246 of 324 | 44 of 68 |
| Keys pressed (median) | 6 | 6 |
| Chaos Inoculation | 67 (20%) | 21 (31%) |
| Life, life builds (median) | 5,123 | 4,558 |
| EHP (median) | 101,759 | 239,922 |
| Mana reserved (median) | 89% | 89% |
| Elemental resistances capped | 266 of 327 | 60 of 68 |
| Flasks with a "Used when" enchant (median) | 3 | 0 |
| Uniques in the 10 gear slots (median) | 4 | 3.5 |

The guides press as many keys and reserve as much mana as the ladder. Fewer of them finish the 6-link. They seldom
show flask enchantments, so do not copy a guide's flasks as the finished setup. EHP depends on the Config tab, so
compare EHP only between builds with the same config.

## Checklist

1. **Main skill in a 6-link** at endgame. 246 of 324 ladder characters do this.
2. **Several reservation skills.** A median 4, and about 90% of mana reserved.
3. **Elemental resistances at 75%.** Check chaos resistance too; 249 of 260 non-CI characters have it at 0% or
   above.
4. **One defence plan.** Life with armour or evasion, CI with energy shield, or low life.
5. **At least +30% movement speed.** 302 of 327 have it. See
   [12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

## Sources

- PoB: `TreeData/3_29/tree.lua` (keystone and Luminary texts), `Data/TimelessJewelData/LegionPassives.lua` and
  `Data/Uniques/jewel.lua` (timeless keystones), `Data/Skills/act_int.lua` and `act_str.lua` (Automation,
  Autoexertion), `Data/Global.lua` (skill types), `Data/Uniques/body.lua` (Kaom's Heart). Classification from
  `build_summary` in `crates/pob-engine/lua/bridge.lua`.
- Corpus: `corpus/poe1/index.json`, `stages.jsonl` and `xml/`, re-read through PoB 2.67.2 to take each socket
  group's skill types and DPS. Usable stages only (no duplicates, no flags).
- poe.ninja: Allflame, Hardcore Allflame and SSF Allflame build snapshots (class and main-skill counts), fetched
  2026-09-26.
- Maxroll: the 27 guides under https://maxroll.gg/poe/build-guides/ that the corpus loads.
