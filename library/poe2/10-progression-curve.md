# Progression curve

What a build should look like at each stage, measured from 324 saved stages
across 63 guides. Use this to sanity-check whether a recommendation is the right
size for the character asking.

## The table

Medians per stage. The level column applies the quest points available at that
point in the campaign (4 per act, 24 by the end of the interludes) to the
observed point count.

| Stage | Files | Tree points | ≈ Level | Weapon-set | Skills | Supports | Gear | Uniques |
|---|---|---|---|---|---|---|---|---|
| Act 1 | 27 | 17 | ~14 | 0 | 6 | 7 | 12 | 0 |
| Act 2 | 20 | 36 | ~29 | 0 | 7 | 10 | 13 | 0 |
| Act 3 | 20 | 52 | ~41 | 0 | 8 | 16 | 13 | 0 |
| Act 4 | 14 | 68 | ~53 | 0 | 8 | 16 | 13 | 0 |
| Interludes | 14 | 83 | ~60 | 0 | 9 | 20 | 14 | 0 |
| Early endgame / maps | 22 | 94 | ~71 | 37 | 10 | 28 | 14 | 3 |
| Endgame | 31 | 100 | ~77 | 48 | 9 | 34 | 15 | 5 |
| Mid endgame | 12 | 99 | ~76 | 48 | 9 | 34 | 15 | 4 |
| Late endgame | 8 | 105 | ~82 | 52 | 11 | 40 | 16 | 8 |
| Uber / mirror tier | 14 | 112 | ~89 | 32 | 10 | 39 | 15 | 6 |

Read the level column as a guide, not a fact. Quest progress and level are only
loosely coupled, and players over- or under-level constantly.

## What the curve says

**Points roughly double from Act 1 to Act 2, then add ~16 per act.** A character
finishing Act 1 has under 20 points. Recommending a build that needs 40 to
function is recommending something they cannot use for another ten levels.

**Skill count is nearly flat.** 6 at Act 1, 9 by the interludes, 9–11 forever
after. The extra skill slots that exist (9 by default) are used early and then
stop being the growth axis.

**Active skills are flatter still.** The Skills column counts socketed gems. Of
those, the number you actually press stays at **4–5 from Act 1 to uber endgame**.
Every gem added after the campaign is a persistent buff, a trigger or a meta gem
— things activated once or fired by a condition. See
[12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

**Supports are the growth axis.** 7 → 40 across the campaign and into endgame,
a near six-fold increase against a two-fold increase in skills. This is where
power comes from.

**Gear slot count barely moves** (12 → 16). What changes is the quality of what
is in those slots, not how many are filled. Only 3–4 slots come online across the
whole campaign.

**Uniques are zero for the entire campaign.** They appear at maps and grow to a
median of 8 in late endgame.

**Weapon-set points are unused until maps.** Zero across every campaign stage in
the dataset.

## Gear turnover points

Levels at which guides most often swap a gear piece, from `level_interval` on
4,124 saved gear entries:

```
1   5   8   10   12   16   20   24   30   32   33   35   40   60   65   70   75   80
```

The heaviest clusters are **level 8, 16, 60 and 80**. The campaign has frequent
small upgrades every 4–8 levels; the endgame has two big steps at 60 and 80 where
item bases change tier.

Practical reading: campaign gear advice should be cheap and expect replacement
soon. Endgame gear advice at level 60 and 80 is where investment is worth it.

## Using this when advising

**Match the stage to the ask.** "A build for a level 21 Ranger" lands between Act
1 and Act 2: about 24–28 points, 6–7 skills, 7–10 supports total, no uniques, no
weapon-set points, tier 1–5 gems.

**State the target explicitly.** "At level 21 you should have around 7 skills
carrying about 10 supports between them" gives the user something to check
against, which a list of gem names does not.

**Do not skip stages.** A level 21 character asking for advice needs the Act 2
shape, and a sentence about what changes at Act 3 — not the endgame tree with a
note that it needs 100 points.

**Flag the swap.** If the archetype changes main skill later (32 of 54 guides
do), say so and give the level.

## Sources

324 `.build` files across 63 guides, analysed 2026-09-03. Stage names classified
from the saved-build titles the authors chose; point counts deduplicated and
split between main tree, weapon set and ascendancy.
