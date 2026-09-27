# Evaluating a change

Guard rails for judging a tree, gem or item change in Path of Exile 1. Every
rule here can be checked with a tool; none of it needs to be worked out in
prose.

Written against 3.29 (PoB 2.67.2, tree 3_29).

## Attribute requirements are a maximum, never a sum

PoB computes the requirement the way the game does (`CalcPerform.lua`): for each
attribute the character needs the **highest single source**, chosen from:

- each equipped item's own requirement;
- each gem's requirement at its current gem level, **supports included**.

Example, from PoB's PoE1 data: Boneshatter at gem level 20 needs **155 Str**, and
Brutality at gem level 20 needs **111 Str**. A character using both needs
**155 Str**, not 266.

What follows from the rule:

- **Gem level sets the requirement.** Every PoE1 gem level carries its own
  character level requirement and attribute cost. A gem the character cannot
  afford is often at too high a level for the stage; `set_gem_levels` lowers
  the gems the level cannot use, supports included.
- **Read it, do not derive it.** `build_summary.requirements` gives `need`,
  `have`, `met` and `from` (the item or gem that sets the number) for each
  attribute. `list_gems` gives each gem's requirement at the gem level the
  character can use, plus `short_by`. Read `requirements` again after any change
  that moves attributes or swaps a gem or an item.
- **Meeting a requirement is cheap.** The tree has **64** small passives that
  give **+10 Strength** (63 for Dexterity, 69 for Intelligence). A Strength
  suffix rolls up to **+55** at item level 82 ("of the Gods") and **+60** on a
  belt at item level 85 ("of the Godslayer"). Each point also carries its
  inherent bonus (see [13-game-constants.md](13-game-constants.md)). Prefer
  those to lowering the main skill's gem level.
- **"Reduced attribute requirements" is usually a dead affix.** It takes a slot
  that could carry the attribute itself. `sanity_check` lists items that carry
  it.

## Flat, increased and more

Damage is `(base + flat added) × (1 + sum of increases) × each more multiplier`.
The three kinds are not interchangeable, and which is worth more depends on
what the build already has:

- **Flat added damage** raises the base. It is worth most when the base is low:
  early levels, a weak weapon, a low-level gem.
- **Increased damage** adds to one pool with every other increase. With 50%
  increased already, another 50% is a third more damage; with 300% already, it
  is an eighth more.
- **More multipliers** (mostly support gems and ascendancy notables) multiply
  everything, which is why a sixth link matters more than several passive
  points. See [09-buildcraft.md](09-buildcraft.md).
- **Local weapon mods** ("increased Physical Damage" on the weapon itself) scale
  that weapon's base before anything else, so they act like a base upgrade.

Do not rank affixes by their text. Equip or craft the candidate and read the
`delta` PoB returns; `optimise_gear` scores every candidate this way. When two
candidates are close, the one that also fixes a requirement, a resistance or a
missing defence layer wins.

## The sheet is not the build

PoB's DPS is one skill's damage under the assumptions in `get_config`. Things it
does not show, and that a player of the build notices at once:

- **Interactions.** A warcry that empowers the next attacks, an aura or herald
  another skill relies on, a curse or exposure another skill applies, a trigger
  that fires a skill for free. Removing the enabler can leave the number
  unchanged only because the config still assumes the effect; clear it with
  `set_config` and read the number again.
- **Uniques as enablers.** A unique whose stats look weak is often worn for a
  line that changes a rule: a conversion, an extra curse, a trigger, a defence
  the class cannot otherwise get. Read the item's text from `get_items` before
  replacing it, and say what the build loses.
- **Gem combos.** A support or a second skill that looks idle on the sheet.
  `skill_info` marks lines PoB does not model as "Not supported in PoB yet"; a
  delta of zero from removing such a gem proves nothing.
- **Uptime and buttons.** A cooldown that leaves gaps, a buff that needs
  re-casting, a skill that only exists to reach the boss. See
  [12-playstyle-and-buttons.md](12-playstyle-and-buttons.md).

Before removing or replacing a unique, a skill or a support, say what it does in
play, and let the user decide when that and the sheet disagree.

## Skills that come with an item

PoB adds a group for every skill an equipped item grants (Death Aura from Death's
Oath, the spiders from Arakaali's Fang), with `grantedBy` naming the item and
slot. When the same skill is also socketed with supports, the item's copy
carries `duplicateOf` pointing at the socketed group. They are one skill; never
count the copy as a second one.

These skills leave with the item and cannot be replaced by a gem. `activeSkills`
leaves them out. Do not spend supports or passives on one unless the build plays
it. To be rid of one, change the item.

A group whose `grantedBy.kind` is `mechanic` is not a skill at all: PoB adds it
to show the damage of a mechanic the build has.

## Reservation is part of the cost

Auras, heralds, golems and stances reserve mana or life. A change that adds one
has to leave enough unreserved mana to pay for the main skill. Read
`manaUnreserved` in `build_summary`; `sanity_check` flags a main skill PoB counts
as unusable. A reservation efficiency line can be worth more than a damage line
when it lets the build run one more aura.

## Stats that are usually dead or off-type

Treat these as a cost, not a bonus, unless the user says the build wants them:

- Reduced attribute requirements (above).
- Attributes far beyond what requirements need, once their inherent bonus is
  counted.
- Off-type damage: spell damage on an attack build, accuracy on a spell build,
  a damage type the main skill does not deal. The tags from `list_gems` say
  what the skill scales with.
- Elemental resistance far over 75%. A margin helps against enemy curses and
  exposure that lower resistance; past that, the affix does nothing.
- Item rarity. PoB gives it no value; the player may want it for farming, so
  say that rather than calling it dead.

## Checklist after any change

Run `sanity_check` again, then confirm from the returned `stats` and `delta`:

1. Elemental resistances still at 75 or above.
2. `requirements.*.met` still true for all three attributes.
3. Life, energy shield and effective hit pool did not fall, or the user asked
   for that trade.
4. The main skill's DPS moved the way the change intended.
5. Unreserved mana still pays for the main skill.
6. Movement speed still present.
7. The number of skills that need a keypress did not grow without a reason.
8. Anything removed was checked with `skill_info` or the item text first.

A change that fails one of these gets rolled back or reversed, and the answer
says why.

## Sources

- PoB PoE1: `Modules/CalcPerform.lua` (requirement is the highest single item or
  gem source), `Modules/CalcTools.lua` (`getGemStatRequirement`),
  `Data/Skills/*.lua` (per-level requirements: Boneshatter and Brutality at gem
  level 20 need character level 70), `TreeData/3_29` (+10 attribute small
  passives), `Data/ModExplicit.lua` (Strength9 "of the Gods" and Strength10 "of the Godslayer").
- PoB Redux: `crates/pob-engine/lua/bridge.lua` (`build_summary`,
  `sanity_check`).
