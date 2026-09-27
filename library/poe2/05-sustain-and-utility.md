# Sustain and utility

Written against 0.5.5.

## Four separate recovery mechanics

Leech, regeneration, recoup and recharge are distinct. **Modifiers to one do not
affect the others**, and flask recovery is none of them. Getting this wrong sends
people chasing the wrong stat.

| Mechanic | Source | Stops at full? | Default window |
|---|---|---|---|
| Leech | Your hit damage dealt | **Yes** | 1 second |
| Regeneration | Flat or % modifiers | No | Continuous |
| Recoup | Damage you took from hits | No | 8 seconds |
| Recharge | Energy shield only | n/a | 12.5%/s after 4s delay |

### Leech

Recovers a percentage of your **hit damage dealt** to a monster. Usually tied to
physical damage by default.

- Base duration **1 second** per instance. Longer duration spreads the same
  recovery over more time; it does not add recovery.
- **All leech instances are removed the moment the resource fills.** This makes
  leech reactive — huge burst recovery when hurt, nothing when healthy.
- Every qualifying hit creates its own instance, with no cap on instances. But
  **only the largest instance recovers at a time**.
- **Hard capped at 40,000 damage per hit** for leech purposes. Scaling damage
  past that adds nothing; you have to invest in leech or recovery modifiers
  instead. Worth knowing before telling an endgame build to keep stacking damage
  for sustain.

### Regeneration

Continuous, never removed at full resource.

- **Life and energy shield have no base regeneration.** Every point comes from
  modifiers.
- **Mana regenerates 4% of maximum per second** by default.
- Most life regeneration on gear is **flat**, not percentage-based. Crafted body
  armours reach 36 life per second on a tier 1 modifier.
- Flat and percentage sources sum into one base, then multipliers apply.

### Recoup

Recovers based on **damage you took from hits**, over **8 seconds** by default.

- Not stopped by a full resource, but the timer keeps running, so recovery past
  full is wasted.
- Recoup modifiers sum: 8% from a notable plus 3% from each of two small passives
  is 14% of damage taken recouped.
- Calculated on damage taken **after** mitigation.
- **Damage over time does not create recoup.**
- "Increased speed of recoup" compresses the window: 25% faster turns 8 seconds
  into 6.4, raising recovery per second without changing the total.

## Flasks

Overhauled from PoE 1. **Two slots only: one life flask, one mana flask.** There
are no utility flasks by name.

- Charges are consumed per use and refilled by **killing monsters**, scaled by
  monster power, or instantly at a **Well** in town.
- Recovery is over the flask's duration, and **stops when the resource fills**.
- Flasks **cannot be rare rarity** — normal, magic and unique only.
- Passive charge generation comes from belts, boots, tree notables (Waters of
  Life, Staunching) and suffixes on magic flasks.
- Vaal Pact prevents life flask use entirely.

## Charms

**Three slots by default**, more from the 'Ancient Vows' quest. Charms trigger
**automatically** when their condition is met — they are the ailment answer, and
they are the thing most low-level builds are missing.

| Charm | Effect | Triggers on |
|---|---|---|
| Ruby | +25% fire resistance | Taking fire damage from a hit |
| Sapphire | +25% cold resistance | Taking cold damage from a hit |
| Topaz | +25% lightning resistance | Taking lightning damage from a hit |
| Amethyst | +18% chaos resistance | Taking chaos damage from a hit |
| Stone | Cannot be stunned | Becoming stunned |
| Silver | Speed unaffected by slows | Being slowed |
| Thawing | Immune to freeze | Becoming frozen |
| Staunching | Immune to bleeding | Starting to bleed |
| Antidote | Immune to poison | Becoming poisoned |
| Dousing | Immune to ignite | Becoming ignited |
| Grounding | Immune to shock | Becoming shocked |
| Golden | 15% increased item rarity | Killing a rare or unique |

Charges work exactly like flask charges, refilled by kills and at Wells, but
modifiers to one do not apply to the other.

Magic charms can roll prefixes granting **guard** or life/mana on use — the
closest thing PoE 2 has to PoE 1's guard skills.

**When someone dies to a specific ailment, the charm is usually the answer**
before any tree or gear change.

## Curses

Spells that debuff everything in an area after a delay.

- **Limit one curse per target** by default. Raising the limit is rare and
  usually carries a downside (Whispers of Doom doubles activation delay).
- **Activation delay is 1.5 seconds** and is unrelated to cast time. Reducible
  with "faster curse activation"; Windscream removes it entirely.
- Curses **can push resistances below 0%**, unlike penetration.

## Marks

**Marks are not curses** and do not count against the curse limit. This is the
main thing to remember — a build can run one curse and one mark.

- Single target. One mark per target, but many targets can be marked.
- Two parts: a debuff while active, and an **activation effect** when a condition
  is met, which consumes the mark. Freezing Mark increases freeze buildup, then
  grants extra cold damage when the target freezes. Sniper's Mark activates on a
  critical hit and adds critical damage bonus to it.
- Scaled by "mark effect" modifiers and, for the debuff duration, by skill effect
  duration.

## Advising on sustain

1. **Match the mechanic to the problem.** Dying to big hits needs a bigger pool
   and mitigation, not leech. Dying to chip damage needs regeneration or recoup.
2. **Leech does nothing at full life** — it is not a defence against burst.
3. **Check the charms** before anything else when a specific ailment is killing
   the character. Three slots, immediate effect, cheap.
4. **Life and energy shield regenerate nothing by default.** Do not assume a
   baseline that is not there.
5. A build can run **one curse plus one mark** — suggesting two curses is wrong
   without a limit modifier.

## Sources

Mobalytics guides, read and condensed: leech, regeneration, recoup, flasks,
charms, curses, mark, energy-shield.
