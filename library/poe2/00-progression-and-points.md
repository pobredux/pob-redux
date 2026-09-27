# Progression: levels, passive points, and permanent rewards

Why this file exists: asked for "a build for an up to level 21 Ranger", the assistant
never set the level and had no idea how many passive points such a character has.
None of this is in PoB's data — PoB knows the max is 123, not how many you have earned.

> **Early access, and this file has a known expiry date.** Written against 0.5.5.
> Path of Exile 2 launches 1.0 on **11 December 2026**, and the campaign
> structure changes: **the interludes go away and the game becomes Acts 1–6**.
> Every quest-point total below is therefore provisional. Re-derive them after
> launch rather than trusting this file.

## Campaign structure

**Now (0.5.5):** Acts 1–4, then the Interludes.

**At 1.0 (11 December 2026):** Acts 1–6. The interludes are removed and their
content becomes part of the act structure. Also announced: the **Duelist** class,
new sword skills, and endgame additions.

PoB's data already anticipates this. `data.worldAreas` tags 23 areas as **Act 6**
— and one of them is literally named `Interlude (Act 6)`, alongside The Refuge,
Holten, The Khari Bazaar, Qimah and Kriar Peaks. The interlude content is sitting
in the Act 6 slot internally. There is **no Act 5** in the data yet.

Two labelling traps when reading external sources:

- **"Act 10" is the Atlas, not a campaign act.** PoB tags 204 areas as act 10 and
  every one is a map — Precursor Tower, Lost Towers, Epitaph, Pit. poe2db's
  "Act10" quest rewards are endgame content for the same reason.
- **poe2db's quest reward table carries PoE 1 class columns** (Marauder, Duelist,
  Shadow, Templar) mixed with PoE 2 ones. Do not read class availability from it.

## Passive points

Two sources, and only the first is arithmetic:

| Source | Amount |
|---|---|
| Character levels | **1 per level after level 1** |
| Campaign quests | **24 across the whole campaign** |

A character starts at level 1 with **zero** points. So:

```
points at level L  =  (L - 1)  +  quest points earned so far
```

**A level 21 character has 20 points from levels.** Quest points depend on progress,
not level, so the total is a range rather than a number. Level 21 usually sits in
Act 2, which means Act 1 is done (4 points) and Act 2 is partly done (0–4):

| Level 21, campaign progress | Passive points |
|---|---|
| Act 1 finished, Act 2 just started | 24 |
| Act 2 finished | 28 |

Quote the range and say what it assumes. Do not present one number as fact.

## Quest rewards by act

Passive points are the headline, but the resistance, spirit and life rewards matter
as much for whether a build works at that stage — a character who has not yet done
Act 1 has no cold resistance from quests and starts every element at ‑50%.

### Act 1 — 4 points
| Quest | Reward |
|---|---|
| Crowbell | +2 passive points |
| Una's Lute | +2 passive points |
| Beira of the Rotten Pack | +10% cold resistance |
| The King in the Mist | +30 spirit |
| Candlemass, the Living Rite | +20 maximum life |

### Act 2 — 4 points
| Quest | Reward |
|---|---|
| Kabala, Constrictor Queen | +2 passive points |
| Shambrin | +2 passive points |
| Sisters of Garukhan Shrine | +10% lightning resistance |
| Valley altar | choice: charm charges / charm slot |

### Act 3 — 4 points
| Quest | Reward |
|---|---|
| Mighty Silverfist | +2 passive points |
| Aggorat altar | +2 passive points |
| Blackjaw, the Remnant | +10% fire resistance |
| Ignagduk, the Bog Witch | +30 spirit |
| Venom Vial | choice: stun threshold / ailment threshold / mana regeneration |

### Act 4 — 4 points
| Quest | Reward |
|---|---|
| Omniphobia, Fear Manifest | +2 passive points |
| Silent Hall | +5% maximum mana |
| Three Trials | three separate choices: +5 attributes **or** +5 elemental resistance each |
| Abandoned Prison | choice: +30% mana **or** +30% life recovery from flasks |
| The Great White One | choice: armour/evasion or hybrid defences |

### Interludes — 8 points total

These become part of Acts 5 and 6 at 1.0. The rewards may survive the
restructure, but where they sit and what the totals are will change.

| Interlude | Reward |
|---|---|
| 1 — Oswin, the Dread Warden | +2 passive points |
| 2 — Akthi and Anundr | +2 passive points |
| 2 — Molten One's Gift | +5% maximum life |
| 2 — Seven Pillars | choice: attributes / resistances / cooldown recovery / movement speed / experience |
| 3 — The Abominable Yeti | +2 passive points |
| 3 — Siege of Oriath | +2 passive points |
| 3 — Lythara, the Wayward Spear | +40 spirit |
| 3 — Elder Madox | a free unique item |

## Spirit

Spirit gates persistent buffs — heralds, auras, some minions. It comes almost
entirely from quests, so a low-level character has very little:

| After | Spirit |
|---|---|
| Act 1 (The King in the Mist) | 30 |
| Act 3 (Ignagduk) | 60 |
| Interlude 3 (Lythara) | 100 |

Recommending two heralds to a character with 30 spirit does not work. Check the
build's Spirit before suggesting anything that reserves it.

## Using this when advising

1. **Set the level.** If a level is asked for, call `set_level` — it changes every
   number PoB reports, and leaving it at 1 makes the whole answer wrong.
2. **State the point budget** and what campaign progress it assumes.
3. **Check Spirit** before suggesting reserved buffs.
4. **Remember starting resistances are ‑50%.** Quest rewards are the first fix
   available, gear the second.

## Sources

- Maxroll, permanent stats from the campaign:
  <https://maxroll.gg/poe2/getting-started/permanent-stats-from-campaign>
- Mobalytics campaign checklist (blocks plain fetching; read in a browser):
  <https://mobalytics.gg/poe-2/guides/0-5-campaign-checklist-stats-rewards-skillpoints-and-more>
