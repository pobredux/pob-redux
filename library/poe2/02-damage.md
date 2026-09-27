# Damage

Written against 0.5.5.

## The five damage types

Physical, fire, cold, lightning, chaos. Fire, cold and lightning are also
"elemental".

| Type | Mitigated by | Notes |
|---|---|---|
| Physical | Armour, additional PDR | No resistance exists for it |
| Fire | Fire resistance | Strength-aligned; left of the tree |
| Cold | Cold resistance | Dexterity-aligned; right of the tree |
| Lightning | Lightning resistance | Intelligence-aligned; top of the tree |
| Chaos | Chaos resistance | Removes **twice** as much energy shield |

Attack skills take their base damage from the weapon. Spells take it from the
gem. Pure physical spells have the highest base critical chance in the game;
pure fire spells the lowest.

## Order damage is calculated in

1. Flat damage — weapon or gem base, plus added damage from gear and supports.
2. Conversion and gain-as-extra.
3. Increases and reductions (summed), then more/less (multiplied).
4. Critical damage bonus.
5. The damage roll, with lucky/unlucky applied here.
6. Double and triple damage chance.

**Increases do not multiply with each other.** "Increased projectile damage" and
"increased lightning damage" go into the same additive bucket. **More** and
**less** modifiers — mostly on skill gems, support gems and ascendancy notables —
are each multiplicative, which is why they are so much stronger per line.

Anything that specifically names a pre-conversion damage type does not scale the
post-conversion damage.

## Conversion and gain

Two distinct mechanics, both applied before any damage scaling. **Neither touches
damage over time** — hits only.

- **Conversion** removes the source damage: "100% of fire damage converted to
  cold" leaves no fire damage.
- **Gain as extra** adds new damage without removing the source: "gain extra fire
  damage from physical damage" keeps the physical damage intact.

Either way, the resulting damage is scaled only by modifiers for the **new** type.

**The two-step process** exists to prevent conversion loops, and it decides what
can chain into what:

- **Step 1 — skill conversion**: conversion or gain written on the skill gem
  itself.
- **Step 2 — everything else**: conversion and gain from gear, the tree, and
  global buffs. All applied *simultaneously*.

Because step 2 is simultaneous, **damage gained in step 2 cannot then be
converted in step 2**. Extra lightning damage from Archmage will not be converted
to cold by Call of the Brotherhood. But a skill with inherent gain (step 1) can
have that gained damage converted afterwards.

If conversion in one step exceeds 100% it is normalised down. 100% physical to
fire plus 50% physical to cold becomes 67%/33%.

## Critical hits

The critical roll happens **when the skill is used**, not per hit. The game rolls
a threshold between 0 and 99.99; your critical chance must exceed it. So a single
Firestorm cast is either fully critical or fully not.

Exceptions that roll per hit: multi-projectile skills (Rain of Arrows) and
channelled skills, which roll per interval or stage. Monsters never roll per hit.

- **Base critical damage bonus is 100%**, so a crit deals twice the base damage.
- Attacks take base crit chance from the **weapon**. Spells, offhand attacks and
  unarmed attacks take it from the **gem**.
- Damage over time cannot crit, but ailments inflicted by a critical hit are
  scaled by it.

**Scaling.** Increased critical chance multiplies your base: 7% base with 75%
increased gives 12.25%. Flat additions to critical chance are far rarer and much
stronger, because they raise the base *before* the multipliers apply — +1% on a
10% base weapon makes every subsequent increase 10% more effective.

Modifiers that prevent critical hits always override modifiers that guarantee
them.

## Accuracy

Only matters for **attack** hits by players. Spells never roll a hit check.

```
Chance to Hit = AA * 1.25 * 100 / (AA + DE * 0.3)
```

Characters gain **+6 accuracy per level** and **+6 accuracy per point of
dexterity**. Chance to hit can reach 100%, but never drops below 5%.

(poe2db states +8 per dexterity. PoB's constant is 6 and that is what the app
reports — see [13-game-constants.md](13-game-constants.md).)

**The distance penalty is the thing people miss.** Players take *less accuracy*
based on range to the target: none within 2 metres, scaling linearly to **90%
less accuracy at 9–12 metres and beyond**. This applies to every player attack
hit, and it is why ranged attack builds need far more accuracy than the character
sheet suggests. Monsters do not suffer it by default.

**Blind** applies 20% less accuracy and 20% less evasion for 4 seconds.

## Penetration

Makes a hit ignore a stated amount of the target's resistance.

**It cannot go below 0% resistance.** This is the important difference from PoE 1
— there is no multiplicative scaling off negative resistance. 75% cold
penetration against a 50%-resistance target wastes 25 of it. Worse, if you have
already dropped the target to ‑4% with curses and exposure, **all** of your
penetration is wasted.

So penetration and resistance-lowering compete rather than combine. Pick one.

Penetration applies to hits only. Ailment damage is calculated pre-mitigation, so
penetration never affects it.

## Speed

**Skill speed** is a generic stat that shortens any skill's animation. It is
**additive** with the specific speed stats — attack speed, cast speed, reload
speed, warcry speed:

```
Final uses per second = base uses per second * (1 + total skill speed%)
```

Skill speed is **not** action speed, which is a separate and much rarer stat.

Skills with fixed added animation time (Rolling Slam) can only have their base
portion scaled; the fixed part is added on afterwards.

## Cooldowns

```
Final cooldown = base cooldown / (1 + increased cooldown recovery rate / 100)
```

100% increased cooldown recovery rate halves the cooldown.

A cooldown is tied to the **skill name**, so the same skill from a different gem
is still blocked. Some supports (Hourglass, Excise) add a cooldown to a skill
that has none — and cannot be used on skills that already have one.

**Cooldown uses** are charges. Gas Grenade has a 3-second cooldown with 3 uses:
you can fire three, and each recovers separately, one at a time. Second Wind
supports and the Grenadier notable add uses.

## Lucky and unlucky

Rolls the outcome twice and takes the better (lucky) or worse (unlucky) result.
They cancel out when both apply.

```
normal average   = (min + max) / 2
lucky average    = (min + 2*max) / 3
unlucky average  = (2*min + max) / 3
```

The swing is one sixth of the damage range, so lucky is worth up to 33% more
damage on a range starting at zero and nothing at all on a flat range. It is
strong on natural lightning damage and weak on physical.

## Charges

Power, frenzy and endurance charges, **maximum 3 each**, lasting **15 seconds**
by default. Gaining another charge of the same type refreshes all of them.

**Charges grant no inherent benefit in PoE 2.** They are fuel — skills, passives
and items consume them or key off how many you have. This is the opposite of
PoE 1, where each charge type carried a built-in bonus.

A build that generates charges without anything that spends or reads them gains
nothing at all. Check the payoff exists before recommending charge generation.

## Limit

Caps how many copies of an effect can persist — "Limit 1 Firestorm". Using the
skill past the limit replaces the oldest instance. Limit **can** be raised by
rare modifiers (Overabundance supports, Multiplying Squalls). "Maximum" is the
similar-sounding keyword that **cannot** be modified.

## Presence

A **4 metre** radius around an entity, with no inherent effect. Items and
ascendancies impose effects inside it.

Presence area is **not** affected by ordinary area of effect modifiers. Only
modifiers that name presence specifically (Enveloping Presence, Alpha's Howl)
change it.

## Tooltip DPS is not real DPS

The skill tooltip omits temporary buffs, curses and exposure on the target, and
on-hit effects like penetration, because their value depends on monster stats.
A build can be considerably stronger than its tooltip. This matters when
comparing two setups by their PoB numbers alone.

It also cuts the other way: a utility support that adds area of effect can beat a
raw damage support when the extra overlaps let a skill hit a single target more
than once.

## Sources

Mobalytics guides, read and condensed: damage-types, damage-conversion,
critical-hits, accuracy, penetration, skill-speed, cooldown, lucky, limit,
presence, damage-defence-calc-order.
