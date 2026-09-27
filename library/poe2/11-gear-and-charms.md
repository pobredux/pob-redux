# Gear slots and charms

What experienced builds actually put in each slot. Counted from the modifier
lines on 4,124 saved gear entries across 63 endgame builds.

## Slot conventions

Each slot has a job. The counts below are how many of the 63 endgame builds carry
that modifier line in that slot.

### Boots — movement speed, then life and resistance

| Count | Modifier |
|---|---|
| **57** | % increased Movement Speed |
| 23 | + maximum Life |
| 22 | % to Fire Resistance |
| 21 | + maximum Energy Shield |
| 21 | % to Lightning Resistance |
| 20 | % to Cold Resistance |
| 17 | % increased Energy Shield |

**57 of 63 builds put movement speed on boots.** It is the most consistent single
choice in the entire dataset — more consistent than any damage stat. Boots
without movement speed is a finding worth raising on any build review.

The reason it outranks damage in this slot: campaign time is mostly spent walking
rather than fighting, and in the endgame mobility is both a defensive layer and
the main driver of clear speed.
[12-playstyle-and-buttons.md](12-playstyle-and-buttons.md) covers this.

### Rings — the flexible slot

| Count | Modifier |
|---|---|
| 44 | + maximum Life |
| 42 | % increased Rarity of Items found |
| 35 | % to all Elemental Resistances |
| 34 | Adds Lightning damage to Attacks |
| 30 | % to Lightning Resistance |
| 27 | Adds Physical Damage to Attacks |
| 26 | + maximum Mana |
| 21 | Adds Fire damage to Attacks |
| 17 | Adds Cold damage to Attacks |
| 14 | % to Chaos Resistance |
| 14 | % increased Cast Speed |
| 13 | + Intelligence |

Rings carry **life, resistances, and flat added damage to attacks**. Flat added
damage on rings is the standard way attack builds raise their base before
multipliers apply.

Item rarity at 42 is notable — nearly two-thirds of endgame builds pay slot
budget for loot rather than power.

### Amulet — spirit and skill levels

| Count | Modifier |
|---|---|
| **29** | + to Spirit |
| 19 | % increased maximum Energy Shield |
| 18 | + maximum Life |
| 15 | + to Level of all Spell Skills |

**Spirit is the amulet's signature stat.** A build short on spirit for its
persistent buffs should look at the amulet first. The amulet is also the anoint
slot — see [06-tree-and-emotions.md](06-tree-and-emotions.md).

### Helmet — energy shield and resistances

| Count | Modifier |
|---|---|
| 27 | + maximum Energy Shield |
| 23 | % increased Energy Shield |
| 21 | + maximum Life |
| 20 | % to Fire Resistance |
| 17 | % increased Rarity of Items found |
| 14 | % to Cold Resistance |
| 13 | % to Lightning Resistance |

### Body armour — the defence base

| Count | Modifier |
|---|---|
| 21 | + maximum Energy Shield |
| 18 | + Evasion Rating |
| 17 | % to Cold Resistance |
| 14 | % to Fire Resistance |
| 14 | % to Lightning Resistance |

The largest defence numbers, plus resistance filler. Note both energy shield and
evasion appear — hybrid bases are common.

### Gloves — life, resistance, added damage, attack speed

| Count | Modifier |
|---|---|
| 28 | + maximum Life |
| 14 | % to Cold Resistance |
| 14 | % increased Energy Shield |
| 13 | Adds Lightning damage to Attacks |
| 13 | + maximum Energy Shield |
| 13 | % increased Attack Speed |
| 13 | % to Fire Resistance |

### Belt — pure defence

| Count | Modifier |
|---|---|
| 23 | + maximum Life |
| 16 | % to Lightning Resistance |
| 14 | % to Cold Resistance |
| 12 | % to Fire Resistance |

Belts carry life and resistance and nothing else. This makes them the natural
place to fix a resistance gap.

### Weapon — the damage base

| Count | Modifier |
|---|---|
| 24 | % increased Physical Damage |
| 21 | % increased Attack Speed |
| 21 | Adds Physical Damage |
| 18 | % increased Spell Damage |
| 13 | Adds Lightning Damage |

Split between attack builds (physical damage, attack speed) and spell builds
(spell damage). Which one applies depends on whether the main skill is an attack
or a spell — see [04-skills-and-gems.md](04-skills-and-gems.md).

### Flasks

| Count | Modifier |
|---|---|
| 21 | % increased Amount Recovered |
| 13 | % reduced Charges per use |

## The resistance pattern

Resistance lines appear across **rings, boots, helmet, body armour, gloves and
belt** — six slots. No single slot solves resistances; they are assembled from
whatever each slot has spare after its main job.

This is why the resistance problem is a *whole-gear* problem. Advice like "get
more fire resistance" is only actionable if it names which slot has room.

## Charms are where the unique budget goes

The most-used unique items in the entire dataset are not weapons or armour. They
are charms and flasks.

| Uses | Item | Type | Effect beyond the immunity |
|---|---|---|---|
| **68** | Lavianga's Spirits | Mana flask | Cannot be used; applies its effect **constantly** |
| **57** | Nascent Hope | Thawing charm | Energy shield recharge starts on use |
| **49** | The Fall of the Axe | Silver charm | Grants Onslaught during effect |
| **41** | Rite of Passage | Golden charm | Possessed by an animal spirit on rare kill |
| **32** | Beira's Anguish | Dousing charm | Ignited ground at 500% of max life as fire damage |
| 31 | Mageblood | Utility belt | |
| 19 | Headhunter | Heavy belt | Gain a rare monster's modifiers for 60 seconds |
| 17 | For Utopia | Stone charm | **Defend with 200% of armour** during effect |
| 14 | Blood of the Warrior | Life flask | Recoup, rage generation, effect not removed at full |
| 14 | Sanguis Heroum | Staunching charm | Consecrated ground on use |

**Five of the top eight uniques are charms.** The pattern is deliberate: a unique
charm gives the ailment immunity a normal charm gives *plus* a large rider, and
it triggers automatically. Three charm slots filled with unique charms is a large
amount of power for very little gear budget.

**Lavianga's Spirits at 68 uses is the single most-used unique in the dataset.**
It converts a flask slot into a permanent buff by being unusable — a constant
effect rather than an active one.

Two practical rules:

1. **Charms are not an afterthought.** They are the most-uniqued slots in the
   game, and the cheapest place to add both an ailment answer and real power.
2. **Check the charm slots on any build review.** Empty or generic charms with a
   known ailment problem is the most common fixable gap.

## Advising on gear

1. **Boots need movement speed.** 57 of 63 agree.
2. **Amulet is the spirit slot.** Short on spirit for heralds? Look here first.
3. **Belt is the resistance slot** with the least competition.
4. **Rings carry flat added damage** for attack builds, and are where rarity goes.
5. **Assemble resistances across six slots**, and name the slot when suggesting a
   fix.
6. **Fill all three charm slots**, and prefer unique charms at endgame.
7. **No uniques during the campaign.** The dataset uses zero.

## Sources

63 endgame `.build` files, modifier lines normalised and counted. Unique item
text resolved against PoB's `data.uniques`.
