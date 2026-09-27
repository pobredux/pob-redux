# Ailments and status effects

Written against 0.5.5.

## The two kinds

**Damaging ailments** deal damage over time: bleeding, ignite, poison.
**Non-damaging ailments** apply a debuff: chill, freeze, shock, electrocute.

An ailment must come from a matching damage type. Without a special item, a hit
that deals only fire damage can never shock.

| Ailment | Damage type | Kind |
|---|---|---|
| Ignite | Fire | Damaging (fire over time) |
| Bleeding | Physical | Damaging (physical over time) |
| Poison | Physical **and** chaos | Damaging (chaos over time) |
| Chill | Cold | Non-damaging |
| Freeze | Cold | Non-damaging |
| Shock | Lightning | Non-damaging |
| Electrocute | Lightning | Non-damaging, **off by default** |

Exceptions exist through items — Voltaxic Rift lets chaos damage contribute to
shock, Blood Barbs lets Blood Mages bleed with elemental damage.

## Rules that apply to all of them

**Ailment threshold.** For most monsters this equals maximum life. Rares and all
unique bosses have adjusted thresholds so players can still inflict ailments on
them. This is why trash freezes instantly and bosses take many hits.

**Buildup is calculated post-mitigation.** Anything that makes the target take
more damage — shock, exposure, curses — also improves your chance to inflict
further ailments. Penetration does not help, because ailment *damage* is
calculated pre-mitigation.

**Magnitude modifiers are multipliers on the base**, not additions. 20% base
ignite magnitude with "25% increased magnitude of ignite" gives 25%, not 45%.

**Magnitude never changes duration**, and duration never changes damage per
second. Longer duration means more total damage; "deals damage faster" means the
same total in less time, so higher damage per second.

**"Increased chance to inflict ailments" is a multiplier** on your existing
chance, not a flat addition. Getting to 100% chance matters enormously for a
build whose damage comes from an ailment.

## Damaging ailments

### Ignite

- Base magnitude **20% of the hit's fire damage per second**, for **4 seconds**.
- Only fire damage in the hit counts.
- **Does not stack.** Only the highest-damage ignite deals damage; the others sit
  there and take over as each expires.
- Chance to ignite comes from **flammability**, a debuff any fire hit applies.
  It lasts 4 seconds, stacks magnitude with each fire hit, and refreshes. Hit
  often enough and flammability reaches 100%, at which point every fire hit
  ignites. Flammability is **not** a curse.
- Ignited ground builds flammability and ignites at 50% magnitude.

### Bleeding

- Base magnitude **15% of the hit's physical damage per second**, over a base
  duration of **5 seconds**. (Mobalytics states 20%; PoB's
  `BleedingHitDamagePercentPerMinute = 900` gives 15%, and PoB is what the app
  calculates with — see [13-game-constants.md](13-game-constants.md).)
- Only physical damage in the hit counts.
- **Does not stack** — same highest-wins behaviour as ignite.
- **Deals 100% extra damage against moving monsters**, or when aggravated. Not
  both — the bonus does not double up. Players bleeding do **not** take extra
  damage while moving.
- **Bypasses energy shield entirely** and hits life directly.
- Corrupted blood is *not* bleeding and shares no stats with it.

### Poison

- Base magnitude **20% of the hit's combined physical and chaos damage per
  second**, dealt as **chaos** damage.
- **Limited to one stack by default.** Raising the limit (Escalating Poison) is
  the main way poison builds scale. All poisons persist; only the highest-damage
  ones up to your limit have any effect.
- **Bypasses energy shield entirely** — the chaos damage from poison goes
  straight to life, unlike ordinary chaos damage which drains double.
- Some skills always poison without any chance investment (Gas Grenade).
- Known bug: modifiers to "effect of poison" should multiply magnitude but are
  currently additive with it.

## Non-damaging ailments

### Chill

- Reduces the target's action speed. Maximum magnitude **50%**.
- **Cold hits chill inherently** — no chance modifier needed.
- **Any chill below 30% magnitude is discarded.** You need enough cold damage
  relative to the target, or magnitude scaling, to land it at all.
- Much easier to apply than freeze.
- Chill from non-damage sources defaults to 20% magnitude.
- Entirely independent of freeze.

### Freeze

- Reduces action speed to **zero** — the target cannot move or act.
- Does **not** interrupt the current action; it pauses and resumes afterwards.
- Higher minimum threshold than chill, so harder to land.
- Duration: from a big enough single hit, scaled by the cold damage taken. From
  buildup reaching 100%, a flat **4 seconds**.
- A freeze below a minimum duration is discarded — which means *increasing*
  freeze duration effectively lowers the threshold needed to freeze at all.

### Shock

- Makes the target take **20% increased damage** by default.
- Additive with other increased-damage-taken effects such as wither.
- Applied post-mitigation, at the end of the damage chain.
- Base chance: **1% per 4% of the target's ailment threshold dealt** by the
  lightning damage in the hit, after mitigation. A hit for 100% of the threshold
  has a 25% base chance.
- Magnitude is **hard-capped at 100% increased damage taken**.
- Duration **8 seconds on monsters, 4 seconds on players**.

### Electrocute

- Interrupts the current action and prevents acting, like stun.
- **Cannot be inflicted at all by default.** It needs an enabler — Kitoko's
  Current, or the Electrocuting Arrow skill. Without one, lightning hits build
  nothing.
- Duration **5 seconds**.
- Shock helps electrocute buildup, since shock lands from ordinary lightning hits
  and raises damage taken.

## Stun

Not an ailment, but the same buildup shape. Hits only — damage over time never
stuns.

- Threshold is **maximum life** by default, adjusted upward for tough monsters.
- **Player physical hits get 50% more stun effectiveness. Player melee hits get
  another 50% more.** Melee physical attacks therefore have 125% more stun
  chance — multiplicative, not additive.
- **Any light stun chance of 15% or less is discarded.**
- Monsters get their own bonuses: 33% more from melee, 100% more from physical.
  This is why packs of melee monsters can chain-stun you to death.
- **Players usually cannot be heavily stunned** — but they can while raising a
  shield, parrying with a buckler, or riding a mount. Minions cannot either.
  Heavy stun lasts **3 seconds** base; an ordinary stun is **500 ms**.
- **Stun has diminishing returns.** Each stun within a 4-second window raises
  your light stun threshold by 50% (final), so chain-stunning gets progressively
  harder.
- Stun threshold can be raised; light stun immunity exists (Unwavering Stance).

## Advising on ailments

- If a build's damage comes from an ailment, **100% chance to inflict it is the
  first priority.** Everything else is secondary.
- Poison and bleeding **bypass energy shield**, so an energy-shield build with no
  chaos resistance or bleed removal is exposed regardless of how large the pool
  is.
- Electrocute needs an explicit enabler — never suggest it without naming one.
- Ailment magnitude scales off the *hit*, so raising hit damage raises ailment
  damage. A build cannot separate the two.
- Chill and shock are cheap and universal; freeze and electrocute need real
  investment.

## Sources

Mobalytics guides, read and condensed: ailments, ignite, bleed, poison, chill,
freeze, shock, electrocute, stun.
