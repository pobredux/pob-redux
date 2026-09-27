# Keyword reference

Short definitions for mechanics that come up in gem and modifier text. Written
against 0.5.5. The bigger topics have their own files; this is the lookup table
for everything else.

## Debuffs you apply to enemies

**Blind** — 20% less accuracy *and* 20% less evasion on the target, 4 seconds.
Cuts both ways in the hit calculation: a blinded enemy hits you less and evades
you less.

**Exposure** — lowers a specific resistance by a set amount. **Stacks additively
with curses** and can push resistance below 0%. Multiple sources of the *same*
exposure do not stack — only the strongest applies — but different types stack
with each other (cold exposure and fire exposure together are fine).

**Withered** — stacking debuff, **up to 10 stacks**, each giving **5% increased
chaos damage taken** (50% at cap). New stacks do not refresh existing ones.
Applied late in the chain, after mitigation.

**Daze** — **50% more stun buildup**, 8 seconds. Multiplicative with the inherent
melee and physical stun multipliers, so it is very strong on a melee physical
build.

**Maim** — 30% reduced movement speed and 15% reduced evasion. Both stack
additively with increases, so 30% reduced cancels 30% increased movement speed.

**Hinder** — 30% reduced movement speed, nothing else. Also additive.

**Pin** — prevents movement, makes the target **unable to evade any hit**, and
grants immunity to knockback. Also applies a light stun. **Not a slow and not an
ailment** — no slow, ailment magnitude or ailment duration modifier affects it.

**Immobilise** — the umbrella state for being unable to move: heavily stunned,
pinned, frozen or electrocuted, or having movement speed reduced to zero (maim
plus hinder can do it). Immobilise itself adds no effect.

**Knockback** — pushes the target away from whatever caused it. Distance is
scalable. **Unique bosses are immune.**

**Slow** — slows action animations. If a slow does not name a stat it applies to
**action speed**, covering movement, attack, cast and skill speed. Multiple slows
on the same stat stack **multiplicatively**, not additively, so they can never
fully stop a target.

**Corrupted blood** — physical damage over time, **stacks up to 10 times**, all
stacking instances dealing damage simultaneously. **Not bleeding** — no bleeding
modifier affects it, and it is not a damaging ailment either.

**Blood loss** — accumulates on the target's life bar from any physical damage
over time, most often bleeding or corrupted blood.

## Offensive mechanics

**Impale** — stores **30% of the pre-mitigation physical damage** of a hit on the
target. Later **attack** hits extract it, adding the stored damage to that hit's
pre-mitigation damage. Any physical hit can inflict it; only an attack hit can
extract it.

**Aftershock** — a repeat hit in the same area for the same damage, following the
initial hit. Mostly on melee slam skills; some supports and notables extend it to
strike skills.

**Culling strike** — instantly kills a target at or below a life threshold. Hits
only, and it **does not count as damage dealt** — it contributes nothing to
overkill.

**Overkill** — damage in a hit beyond what was needed to kill. A 7,000 hit on a
target needing 5,000 has 2,000 overkill.

**Rage** — stacking buff, **1% more attack damage per stack**, default maximum 30
stacks for 30% more. Decays if you have not taken damage or gained rage in 4
seconds. Generation is mostly tied to melee, but the buff applies to **all**
attack damage including projectiles — so it fits any attack build that can
sustain it.

**Detonator** — a skill tag. Detonator skills set off ground surfaces created by
other skills. Explosive Grenade detonating Gas Grenade's poison clouds is the
canonical pair: it deals immediate damage but expires the cloud early.

**Elemental infusions** — fire, cold or lightning remnants dropped by certain
skills, collected by walking over them, stored as a counter on the character.

## Ground surfaces

Areas that apply effects to entities standing in them — buffs to the creator's
side, debuffs to their enemies, occasionally both.

Environmental surfaces (chilled ground from a map modifier) have no creator and
count as **enemy** surfaces for interaction purposes.

**Poison cloud** — created by Gas Grenade, Gas Arrow, Decompose. Poisons monsters
inside **every 2 seconds, as though hitting**.

**Ignited ground** — builds flammability, igniting at 50% magnitude.

## Defensive actions

**Dodge roll** — available to every character, no cost and no cooldown. **Not a
skill**, so skill speed does not affect it — but **action speed does**. Grants
avoidance at the start of the roll.

**Parry** — on blocking with the Parry skill you retaliate with an offhand attack
based on the **inherent evasion of an equipped buckler**. That attack **cannot be
evaded** and has **400% more stun buildup**.

## Resource and state keywords

**Low / High / Full** — thresholds referenced by modifiers, describing the state
of a resource.

**Overflow** — lets a resource exceed its normal maximum. Note that modifiers
scaling from a maximum resource **do not count overflowed amounts**.

**Immune** — the effect cannot be applied at all.

**Unaffected** — the effect applies but has no impact. Different from immune, and
occasionally the distinction matters for mechanics that check whether a debuff is
present.

**Expiration rate** — makes time pass faster or slower for specific buffs and
debuffs. Temporal Chains slowing an electrocute's effective duration is an
example.

**Incision** — a bleeding-related debuff.

**Gruelling madness** — a debuff tied to Delirium items.

## Sources

Mobalytics guides, read and condensed: blind, exposure, withered, daze, maim,
hinder, pin, immobilise, knockback, slow, corrupted-blood, blood-loss, impale,
aftershock, culling-strike, overkill, rage, detonator, elemental-infusions,
ground-surfaces, poison-cloud, dodge-roll-mechanic, parry, low-life-high-full,
overflow, immune, unaffected, expiration-rate, incision, gruelling-madness.
