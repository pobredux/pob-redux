# Defences

Written against 0.5.5. Numbers change between patches — re-check after a major one.

## The order damage is processed in

Everything below sits somewhere in this chain. Knowing the order tells you which
layers stack and which are wasted.

1. **Avoid the hit** — evasion, dodge roll invulnerability, avoidance that names a
   hit type. An avoided hit stops here and never enters the rest of the chain.
2. **Calculate damage** — flat damage, then conversion and "gained as extra",
   then increases/more, then critical bonus, then the damage roll, then
   double/triple damage.
3. **Damage taken as** — shift a portion to another type. Can only happen once.
4. **Mitigation** — immunities, damage avoidance, then armour and additional
   physical damage reduction, then resistances.
5. **Damage taken modifiers** — flat, then increased/reduced (summed), then
   more/less (multiplied).
6. **Stun** check.
7. **Block** roll.
8. **Resource loss** — energy shield, then life, then runic ward.

Damage over time skips every hit-only step (1, 6, 7).

Two things follow from the order and matter constantly:

- **Damage reduction and resistances are separate layers**, each capped at 90%.
  They multiply against each other, so both are worth having.
- **Ailment magnitude is calculated before mitigation.** Stacking armour does not
  reduce the size of the shock or ignite a hit applies.

## Armour

Mitigates physical hit damage. The reduction is not flat — it depends on how big
the hit is relative to your armour:

```
Damage Reduction = AR / (AR + 10 * DMG)
```

Which means armour is good against many small hits and poor against a few large
ones. Useful ratios:

| To mitigate | Armour needed |
|---|---|
| 33% | 5x the hit |
| 50% | 10x the hit |
| 66% | 20x the hit |
| 75% | 30x the hit |
| 90% | 90x the hit |

- Caps out at mitigating **one fifth of its own value** in a single hit. 20,000
  armour stops at most 4,000 damage from one hit.
- Physical damage reduction is hard-capped at **90%** from all sources.
- **Does nothing against damage over time.** Bleed and corrupted blood go through
  it untouched.
- Some modifiers apply armour to elemental damage (the Heatproofing notable,
  Blackbraid). When they do, armour is applied *before* resistance, and each
  damage type gets its own separate calculation.

**Additional physical damage reduction** (PDR) is different: a flat percentage,
added directly to the armour result, and unlike armour it *does* apply to
physical damage over time. 10,000 armour against a 1,000 hit gives 50%; add 8%
PDR from a shield and you take 58% less.

**Armour break** removes a flat amount of a target's armour for 12 seconds.

## Evasion

Grants a chance to evade an incoming hit outright. An evaded hit is discarded and
never enters the rest of the chain — the strongest form of mitigation there is,
when it works.

```
Chance to Evade = (1 - (AA * 1.25) / (AA + DE * 0.3)) * 100
```

`AA` is the attacker's accuracy, `DE` your evasion. Capped at **95%**.

- Does not work against **unavoidable telegraphed boss abilities**.
- **Does not interact with damage over time.** It can still prevent a bleed or
  poison indirectly, because those need a hit to land first.
- Dodge-roll avoidance is not evasion, though both sit at step 1.

**Entropy.** Evasion is not a per-hit coin flip. The game rolls a hidden value
0-99 once every 100 server ticks (about 3.33 seconds), then adds the attacker's
chance to hit to it. At 100 or more the hit lands and 100 is subtracted; below
100 the hit is evaded and nothing is subtracted. The practical effect is no lucky
or unlucky streaks — evasion behaves close to its stated percentage over any
short window. Entropy is one value on the character, shared across everything
hitting you.

## Deflection

Rolled separately from and in addition to evasion, so it stacks as its own layer.

```
Chance to Deflect = 150 * (1 - A / (A + 0.12 * D))
```

Capped at 95%. A successful deflect reduces the hit's damage by **40%** by
default, and also reduces damaging ailments that hit inflicted. Unlike evasion it
works against **unavoidable boss abilities**, which makes it valuable exactly
where evasion fails.

Sourced as a percentage of evasion on dexterity gear, so it needs heavy evasion
investment to be worth anything.

## Energy shield

A resource that sits in front of life. Damage hits it first.

- **Chaos damage removes twice as much** energy shield.
- **Bleed and poison bypass it entirely** and go straight to life.
- Recharge: **12.5% per second**, starting after **4 seconds** without losing
  energy shield to damage. Any damage interrupts it and restarts the delay.
- "Faster start of recharge" shortens the delay by altering elapsed time —
  100% faster means the delay is 2 seconds, not 4.
- Recharge is recovery, **not regeneration**. Modifiers naming regeneration do
  not apply to it.

**Local vs global** is the trap. `X% increased Energy Shield` on a helmet is
local — it scales that helmet's own energy shield. The same text on an amulet is
global, because an amulet has no inherent energy shield for it to apply to. When
reading gear, check whether the item has the base stat.

## Resistances

- Cap is **75%** by default, raised by "maximum resistance" modifiers, hard-capped
  at **90%**.
- Exist for fire, cold, lightning and chaos. Physical has no resistance — it uses
  armour and PDR instead.
- **Characters start at -50% to each element.** The first fixes are quest rewards
  (see [00-progression-and-points.md](00-progression-and-points.md)), then gear
  suffixes.
- Curses and exposure can push a target below 0%, down to a floor of **‑200%**.
  **Penetration cannot** — it stops at 0%.

Getting all three elements to 75% is the highest-value defensive goal for any
build, and it is the first thing to check before suggesting anything else.

## Block

Prevents **all** of a hit's damage on a successful roll.

- Base chance comes from a shield, or rarely another item.
- **Capped at 90%.**
- **Only covers a 210-degree arc.** Attacks from behind cannot be blocked at any
  block chance. A block-stacking build still has a back.
- **Passive block is pure chance**, per hit, with no entropy system behind it.
- Blocked hits still count as hits: they still apply stun buildup, freeze buildup
  and other on-hit effects, because the hit already passed through the damage
  calculation before block was rolled.
- **You cannot block while frozen or stunned.**
- Some boss abilities cannot be blocked — signalled by a red flash and audio cue.
- Damage over time cannot be blocked.

## Guard

A temporary buff holding a pool of damage absorption, spent before life or energy
shield. **Only one guard buff can be active at a time.** Hit damage only — it does
nothing against damage over time. Magnitude and duration come from whatever
granted the buff.

## Runic ward

A pool *below* life. Once life reaches 1, further damage eats runic ward; when
both run out you die.

- Regenerates at **5% per second**, independent of life.
- Added via the Runeforging bench. On items of **level 55 or below it is pure
  upside** — added on top of existing defences. Above 55 the item trades armour,
  evasion or energy shield for it.
- Also adds to starting Resolve in the Trial of the Sekhemas.

## Damage taken as

Shifts a portion of incoming damage to another type, before mitigation. The
shifted portion loses everything specific to its original type — physical taken
as fire is no longer reduced by armour, and cold taken as fire loses the
attacker's cold penetration.

Can only shift **once**; all such modifiers apply simultaneously.

Pairs well with armour: shifting part of a physical hit to fire leaves a *smaller*
physical hit behind, and armour's formula is better against small hits.

## Building defences

The guides frame it as layers, and that ordering makes a good checklist:

1. **Avoidance** — evasion, block, movement speed, dodge rolling.
2. **Mitigation** — armour/PDR, resistances, deflection, damage taken as.
3. **Health pool** — life, energy shield, runic ward.
4. **Recovery** — regeneration, leech, recharge, flasks.
5. **Damage** — a dead enemy deals none.

A build missing a whole layer is fragile even if one layer is large. When
reviewing someone's build, look for the gap before suggesting more of what they
already have.

## Sources

Mobalytics guides, read and condensed: defences-basics, armour, evasion,
deflection, energy-shield, resistances, block, guard, runic-ward,
damage-taken-as, damage-defence-calc-order.
