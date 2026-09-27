# Evaluating a change

Guard rails for judging a tree, gem, or item change. Written after the assistant
summed attribute requirements that do not add, chased sheet DPS at the cost of
what made a build work, and treated a filler affix as an upgrade. Every rule
here is checkable with a tool; none of it needs to be worked out in prose.

## Attribute requirements are a maximum, never a sum

PoB computes the requirement exactly the way the game does (CalcPerform.lua):
for each attribute the character needs the **highest single source**, chosen
from:

- each equipped item's own requirement;
- each skill gem's requirement at its current gem level;
- one shared **"Support Gems"** source per colour: 5 × the number of supports of
  that colour socketed in enabled groups, across the whole build.

Example: a mace needing 80 Str, Earthquake at gem level 10 (65 Str), and four
red supports (one source of 20). The character needs **80 Str**. Not 165.

What follows from the rule:

- **Support gems have no requirement of their own.** Their only effect is the
  shared per-colour source, and it binds only when 5 × count is higher than
  every item and skill gem source. Four blue supports on a strength character is
  20 Int, which is a real cost at level 10 and nothing once an amulet carries
  intelligence. Five red supports on a strength build change nothing.
- **Gem level sets the requirement.** Earthquake needs 65 Str at gem level 10
  and 157 at gem level 20. A gem the character cannot afford is often a gem at
  too high a level for the stage, and `set_gem_levels` or a lower `level` on
  `add_gem` fixes it without touching gear. `add_gem` already defaults to the
  highest gem level the character level allows.
- **Read it, do not derive it.** `build_summary.requirements` gives `need`,
  `have`, `met` and `from` (the one item, gem or support-gem source that sets
  the number) for each attribute. `list_gems` gives each gem's requirement at
  the gem level the character can use, plus `short_by`. After any change that
  moves attributes or swaps a gem or item, read `requirements` again.
- **Meeting a requirement is cheap.** A travel node is +5 to an attribute and
  carries an inherent bonus (see
  [13-game-constants.md](13-game-constants.md)); a suffix on any jewellery slot
  is +20 to +30. Prefer those to lowering the gem level when the gem is the
  main skill.
- **"Reduced attribute requirements" is usually a dead affix.** It occupies a
  suffix that could carry the attribute itself, which meets the requirement
  and adds life, accuracy or mana on the side. Keep it only when the item is
  otherwise the best available and nothing else would meet the requirement.
  `sanity_check` lists items that carry it.

## Flat, increased, and more

Damage is `(base + flat added) × (1 + sum of increases) × each more multiplier`.
The three kinds are not interchangeable and which one is worth more depends on
what the build already has:

- **Flat added damage** raises the base. It is worth the most when the base is
  low: early levels, a weak weapon, a low-level gem. Its value is multiplied by
  every increase and more the build already has, so it also stays useful late.
- **Increased damage** scales the base additively with every other increase.
  With 50% increased already, another 50% is a third more damage; with 300%
  already, another 50% is an eighth more. Once a build has a high pool of flat
  damage and a good base, increases are where the tree and gear spend, and
  their value keeps shrinking as more are added.
- **More multipliers** (mostly support gems, ascendancy notables, and some
  uniques) multiply everything. They are the strongest line per line and the
  reason support choice matters more than passive count
  ([09-buildcraft.md](09-buildcraft.md)).
- **Local weapon mods** ("increased Physical Damage" on the weapon itself)
  scale that weapon's base before anything else, so they act like a base
  upgrade rather than a global increase. A weapon with high local damage and
  flat added is what attack builds scale from.

Do not rank affixes by their text. Equip or craft the candidate and read the
`delta` PoB returns; `optimise_gear` scores every candidate this way already.
When two candidates are close, the one that also fixes a requirement, a
resistance, or a missing defence layer wins.

## The sheet is not the build

PoB's DPS is one skill's damage under the assumptions in `get_config`, on a
target with the configured resistances. Things it does not show, and that a
player of the build will notice at once:

- **Interactions.** A warcry that empowers the next slam, a cry or shock that
  sets up a herald, armour break that the main hit relies on, a curse or
  exposure that another skill applies, a trigger gem that fires a skill for
  free. Removing the enabler leaves the number where it was only because the
  config still assumes the effect; clear the assumption with `set_config` and
  read the number again.
- **Uniques as enablers.** A unique whose stats look weak is often worn for a
  line that changes a rule: a conversion, "counts as", an extra curse, a
  trigger, a defence the class cannot otherwise get. Read the item's text from
  `get_items` and the main skill's text from `skill_info` before replacing it,
  and say what the build loses if the line is what the skill setup depends on.
  A rare with more life is not an upgrade over a unique the build was built
  around.
- **Gem combos.** The same applies to a support or a second skill that looks
  idle on the sheet. `skill_info` marks lines PoB does not model as "Not
  supported in PoB yet"; a delta of zero from removing such a gem proves
  nothing.
- **Uptime and buttons.** A cooldown that leaves gaps, a buff that needs
  re-casting, a skill that only exists to reach the boss. See
  [12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

So before removing or replacing a unique, a skill, or a support, say what it
does in play, not only what the sheet says, and let the user decide when the
two disagree.

## Skills that come with an item

Two kinds of skill are in a build because of an item, not because the player
chose a gem:

- **Default weapon attacks and item skills.** Mace Strike, Quarterstaff Strike,
  Bow Shot, Crossbow Shot, Spear Stab, Raise Shield, and skills a unique grants.
  PoB gives them tier 0 because no uncut gem makes them. The game lists them
  among the character's skills and lets supports sit on them, so an import
  socket group can hold one with supports. Such an entry carries `granted`
  ("default attack of One Hand Mace; comes with the weapon, not a gem").
- **The item's own copy.** PoB also adds a support-less group for every skill an
  equipped item grants, with `grantedBy` naming the item and slot. When the
  same skill is socketed as well, that copy carries `duplicateOf` pointing at
  the socketed group, and the socketed group carries `grantedCopy`. They are
  one skill; the socketed group is where the supports live. Never count or
  describe the copy as a second skill.

Both kinds leave with the item and cannot be removed or replaced by a gem. Their
presence says nothing about whether the build uses them: a level 90 Titan does
not press Mace Strike. `activeSkills` leaves them out and `grantedSkills` counts
them. Do not propose swapping them out, and do not spend supports or passives
on them unless the build plays them. To be rid of one, change the item.

A group whose `grantedBy.kind` is `mechanic` (Thorns, Explode) is not a skill
at all: PoB synthesises it so it can show the damage of a mechanic the build
has. It costs nothing, cannot be pressed, and is not something to "fix".

## Stats that are usually dead or off-type

Treat these as a cost, not a bonus, unless the user says the build wants them:

- Reduced attribute requirements (above).
- Attributes far beyond what requirements need, once their inherent bonus is
  counted. An extra 30 Int on a pure strength build is 60 mana and nothing
  else.
- Off-type damage: spell damage on an attack build, weapon elemental damage on
  a spell build, accuracy on a spell build, a damage type the main skill does
  not deal. The tags from `list_gems` say what the skill scales with.
- Elemental resistance far over 75%. A margin of 10 to 20 points covers map
  mods; beyond that the affix is doing nothing.
- Thorns and reflect-style damage, unless the build is built around them.
- Item rarity. PoB gives it no value; the player may want it for farming, so
  say that rather than calling it dead.

## Checklist after any change

Run `sanity_check` again, then confirm from the returned `stats` and `delta`:

1. Elemental resistances still at 75 or above.
2. `requirements.*.met` still true for all three attributes.
3. Life, energy shield and effective hit pool did not fall, or the user asked
   for that trade.
4. The main skill's DPS moved the way the change intended.
5. Spirit still covers every reservation; mana cost still sustainable.
6. Movement speed on boots still present.
7. The number of skills that need a keypress did not grow without a reason.
8. Anything removed was checked with `skill_info` or the item text first.

A change that fails one of these gets rolled back or reversed, and the answer
says why.

## Sources

PoB's `Modules/CalcPerform.lua` (requirement calculation), `Modules/CalcSetup.lua`
(support-gem requirement source), `Modules/CalcTools.lua`
(`getGemStatRequirement`), and the damage order in
[02-damage.md](02-damage.md).
