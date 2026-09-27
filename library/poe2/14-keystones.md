# Keystones

All 33 keystone passives, with the downside stated. Keystones are the
build-defining choices on the tree, so "which keystone does this build want" is
usually a better opening question than "which notables are good".

Written against 0.5.5, from poe2db. There are also 8 timeless jewel keystones not
listed here.

## Damage and conversion

**Resolute Technique** — Accuracy rating is doubled. *Never deal critical hits.*
Solves the accuracy falloff problem outright; ends any critical scaling.

**Avatar of Fire** — 75% of damage converted to fire. *Deal no non-fire damage.*

**Blackflame Covenant** — Fire spells convert 100% of fire damage to chaos.
Chaos damage from fire spells contributes to flammability and ignite magnitude.
Ignite from fire spells deals chaos damage instead of fire.

**Elemental Equilibrium** — Rotates infusion remnants: lightning instead of fire,
cold instead of lightning, fire instead of cold.

**Pain Attunement** — 30% more critical damage bonus on low life. *30% less on
full life.* Low life is **35%**, not half — see
[13-game-constants.md](13-game-constants.md).

**Crimson Assault** — Bleeding you inflict is aggravated, 50% more bleeding
magnitude. *Base bleeding duration is 1 second* (down from 5).

**Primal Hunger** — 100% more maximum rage, regenerate 1 rage per second per 4
rage spent recently. *No rage effect* — rage stops granting attack damage and
becomes pure fuel.

**Dance with Death** — 25% more skill speed while your off hand is empty and a
one-handed martial weapon is in your main hand.

## Resources

**Blood Magic** — You have no mana; skill mana costs become life costs.

**Chaos Inoculation** — Maximum life is 1. **Immune to chaos damage and
bleeding.** The bleeding immunity matters as much as the chaos immunity, because
bleed and poison both bypass energy shield.

**Eldritch Battery** — Convert 100% of maximum energy shield to maximum mana.
*Mana costs are doubled.*

**Mind Over Matter** — All damage is taken from mana before life. *50% less mana
recovery rate.*

**Zealot's Oath** — Excess life recovery from regeneration applies to energy
shield. *Energy shield does not recharge.*

**Eternal Youth** — Life recharges instead of energy shield. *50% less life
recovery from flasks.* Life recharge uses the same 12.5% per second after 4
seconds as energy shield.

**Scarred Faith** — 5% of physical damage prevented recouped as energy shield per
enemy power. *Energy shield does not recharge, and cannot be recovered from
regeneration.*

**Vaal Pact** — 50% more life leeched, and leech is not removed when unreserved
life is filled. *Leech life 67% less quickly. Cannot recover life other than from
leech* — which rules out flasks, regeneration and recoup.

**Oasis** — 30% more recovery from flasks. *Cannot use charms.* A direct trade
against the charm slots that [11-gear-and-charms.md](11-gear-and-charms.md) shows
are the most-uniqued in the game.

## Defence

**Unwavering Stance** — Cannot be light stunned. *Cannot dodge roll or sprint.*
Gives up a primary avoidance layer for a stun answer.

**Iron Reflexes** — Converts all evasion rating to armour.

**Glancing Blows** — Chance to evade is unlucky; chance to deflect is lucky.
Trades the reliability of evasion for better deflection, which unlike evasion
works against unavoidable boss abilities.

**Bulwark** — Take 30% less damage from hits while dodge rolling. *Dodge roll
cannot avoid damage.* Turns dodge roll from avoidance into mitigation.

**Heartstopper** — Take 50% less damage over time if you started taking damage
over time in the past second. *Take 50% more if you did not.* Rewards constant
low-level damage over time exposure and punishes sudden spikes.

## Equipment and skills

**Giant's Blood** — You can wield two-handed axes, maces and swords in one hand.
*Triple attribute requirements of martial weapons. Inherent life granted by
strength is halved.*

Two downsides, not one, and both are severe. Tripled attribute requirements is a
large travel-node bill; halved strength-life turns +2 life per strength into +1.

**Hollow Palm Technique** — Attack as though using a quarterstaff while both hand
slots are empty. Unarmed attacks that would use an equipped quarterstaff's damage
instead get base damage from skill level, 1% more attack speed per 75 item
evasion on equipped armour, and +0.1% critical chance per 10 item energy shield
on equipped armour.

**Ancestral Bond** — Totem limit doubled, no charge requirement to place totems.
*Totems reserve 75 spirit each.* At 100 maximum spirit, one totem is nearly the
whole budget.

**Ritual Cadence** — Invocation skills trigger spells every 2 seconds instead,
and invoked spells consume 50% less energy. *Invocation skills cannot gain energy
while triggering.*

**Trusted Kinship** — Two companions of different types, 30% more reservation
efficiency of companion skills. *20% less reservation efficiency of
non-companion skills.*

## Curses, charges and minions

**Whispers of Doom** — Apply an additional curse. *Double curse activation
delay*, so 3 seconds instead of 1.5.

**Necromantic Talisman** — All bonuses from your equipped amulet apply to your
minions instead of you. Costs the spirit that the amulet usually provides — see
[11-gear-and-charms.md](11-gear-and-charms.md).

**Conduit** — If you would gain a charge, allies in your presence gain it
instead.

**Resonance** — Rotates charge types: power instead of frenzy, frenzy instead of
endurance, endurance instead of power.

Charges grant no inherent benefit in PoE 2, so charge keystones only pay off
alongside something that spends or keys off the charge type you end up with.

## Using keystones in advice

1. **Name the downside every time.** Every keystone has one, and several have
   two. A recommendation that mentions only the upside is not usable.
2. **Check the interaction with the build's existing plan.** Vaal Pact plus a
   life flask, Oasis plus unique charms, Ancestral Bond plus heralds — these are
   direct conflicts.
3. **Keystones are reachable early.** Published trees allocate them during the
   campaign, so "what keystone are we heading for" is a level 20 question, not a
   level 80 one.
4. **Respec costs gold**, so a keystone is a commitment but not a permanent one.

## Keystones that change what to recommend

Some keystones make whole groups of lines useless or change what they do.
`build_summary` returns the first five as `keystoneRules` when the build has
them, including when an item grants the keystone.

- **Chaos Inoculation.** Life stays at 1, so life lines, life regeneration, life
  leech and life flasks do nothing. Chaos resistance does not matter. Raise
  energy shield instead. Conditions "while on Full Life" are always met.
- **Eldritch Battery.** Energy shield lines raise mana, so they stay useful
  although energy shield reads 0. Without Mind Over Matter the build has no
  energy shield between hits and its life.
- **Mind Over Matter.** Mana is a defensive pool. Mana lines count as defence,
  and mana recovery keeps the character alive as well as paying costs.
- **Blood Magic.** There is no mana. Mana lines, mana regeneration and mana cost
  reduction do nothing, and life pays for skills.
- **Iron Reflexes.** Evasion lines become armour, so an evasion base is not an
  off-type item.
- **Zealot's Oath and Scarred Faith.** Energy shield does not recharge, so
  recharge lines do nothing. **Eternal Youth** moves recharge from energy shield
  to life.

## Sources

poe2db Keystone page (33 keystone passives, 8 timeless jewel keystones), read
2026-09-03. Numeric details cross-checked against
[13-game-constants.md](13-game-constants.md).
