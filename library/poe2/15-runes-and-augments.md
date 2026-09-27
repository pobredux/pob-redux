# Runes, soul cores and augments

A whole mechanic the library was missing. Runes are free power on any gear piece
with an open socket, and they are available from Act 1.

Written against 0.5.5, from poe2db.

## Augments

**Augment** is the umbrella term for anything socketed into an augment socket on
equipment. Two kinds exist:

- **Runes** — Kalguuran make. The common kind.
- **Soul cores** — artifacts of the ancient Vaal empire.

Rules that apply to both:

- Socketed into **augment sockets**, usually on equipment.
- Once socketed, an augment **can be replaced by another augment but cannot be
  removed**. There is no un-socketing.
- **Socket-bound augments** are a special case: they permanently fill the socket
  they go into and cannot be replaced at all.

The no-removal rule makes socketing a commitment. Advise filling sockets on gear
the character intends to keep, not on a levelling piece about to be replaced.

## What a rune does depends on the item

This is the part that surprises people. **One rune gives three different effects**
and which you get depends on what you socket it into:

| Item type | Gets |
|---|---|
| Martial weapon | An offensive stat, usually flat damage or leech |
| Wand or staff | A caster equivalent, usually gain-as-extra or a resource |
| Armour | A defensive stat, usually resistance, life or energy shield |

So the same Desert Rune is added fire damage in an axe, gain 8% of damage as
extra fire in a wand, and +14% fire resistance in a chest.

**Never recommend a rune without naming the slot.** "Put a Storm Rune in" means
three different things.

## Bonded

Each rune also has a **Bonded** effect, a second and usually stronger line. The
armour Bonded effect is the same across every rune listed — **+20 maximum life
and +20 maximum mana** — while weapon and caster Bonded effects are
rune-specific.

## The elemental runes

| Rune | Martial weapon | Wand or staff | Armour | Bonded (weapon/caster) |
|---|---|---|---|---|
| Desert | Adds 7–11 fire damage | Gain 8% of damage as extra fire | +14% fire resistance | 30% increased ignite magnitude |
| Glacial | Adds 6–10 cold damage | Gain 8% of damage as extra cold | +14% cold resistance | 30% increased freeze buildup |
| Storm | Adds 1–20 lightning damage | Gain 8% of damage as extra lightning | +14% lightning resistance | 30% increased shock magnitude |

Storm Rune's 1–20 range is very wide, which makes it the natural target for
**lucky damage** modifiers — see [02-damage.md](02-damage.md), where lucky is
worth up to 33% more damage on a range starting near zero and nothing on a flat
one.

## The utility runes

| Rune | Martial weapon | Wand or staff | Armour |
|---|---|---|---|
| Iron | 16% increased physical damage | 25% increased spell damage | 16% increased armour, evasion and energy shield |
| Body | Leeches 4% of physical damage as life | +40 maximum energy shield | +45 maximum life |
| Mind | Leeches 3% of physical damage as mana | +60 maximum mana | +30 maximum mana |
| Rebirth | Gain 25 life per enemy killed | 8% increased energy shield recharge rate | Regenerate 0.4% of maximum life per second |
| Inspiration | Gain 20 mana per enemy killed | 25% increased mana regeneration rate | 15% increased mana regeneration rate |

Bonded effects on these: Iron gives increased effect of fully broken armour
(weapon) or armour break on spell critical (caster); Body and Mind give 5%
increased maximum life and mana respectively; Rebirth gives life regeneration or
8% of damage taken recouped as life.

Other runes exist — Adept, Charging, Resolve, Robust, Stone, Tempered, Vision,
Ward, Warding, Ancient, Masterwork — following the same three-way pattern.

## Tiers and level requirements

| Tier | Requires level |
|---|---|
| Lesser | Act 1 quest reward |
| (base) | **15** |
| Greater | **30** |
| Perfect | **50** |

A few runes are gated higher regardless of tier — Adept and Charging require
level 50 at base.

**Lesser runes are an Act 1 quest reward** (Lesser Desert, Glacial, Storm and
Iron). So runes are available from the first act, which makes them the cheapest
power on a levelling character. Nothing in the campaign builds analysed used
uniques; runes are what fills that gap.

## Runic ward and the Runeforging bench

Separate from socketing runes, and easy to confuse. The **Runeforging bench**
converts a gear piece to carry runic ward:

- **Items level 55 or below gain runic ward on top of existing defences** — pure
  upside.
- Above level 55, the item trades armour, evasion or energy shield for it.
- Once runeforged, the item cannot be modified by that bench again.

See [01-defences.md](01-defences.md) for what runic ward does.

## Advising on runes

1. **Check for empty augment sockets** on any build review. They are free stats
   and commonly left empty.
2. **Name the slot**, because the effect changes with item type.
3. **Do not socket a levelling piece** you expect to replace — augments cannot be
   removed.
4. **Match the rune to the scaling.** Elemental runes on a weapon give flat
   damage that multiplies with everything downstream; the same rune on armour is
   pure resistance.
5. **Resistance runes are a cheap resistance fix**, at +14% each, and worth
   raising before suggesting a gear rewrite. Three armour pieces with elemental
   runes is +42% spread across the elements.
6. **Level 15, 30 and 50** are the upgrade points.

## Sources

poe2db Rune and Soul Core pages, plus the Augment and Socket-bound Augment
keyword entries, read 2026-09-03. Quest reward levels from poe2db's QuestRewards
page.
