# Playstyle: buttons, automation and movement

Two things that drive real build decisions and do not show up in any stat block.
The first is measured from the build files; the second is why a measured number
looks the way it does.

## Button count is a design goal

Players want to press fewer keys. "One-button build" is a recognised category,
and several guides in the dataset say so in their titles: *Ulf's Titan 1-BUTTON
CASTER*, *Supporting Fire Only Tact - One Button King*, *Temporalis Infinite Loop
Autobomber*, *Infernalist Auto Spark CoA Comet*.

This matters because a build that is strong on paper and needs seven keys in
rotation is worse, to most players, than a slightly weaker build that needs two.

### Not every socketed gem is a button

Classifying every skill gem by PoB's own tags splits them four ways:

| Class | Gems | Needs a keypress? |
|---|---|---|
| Active | 237 | **Yes**, every use |
| Persistent | 93 | No — activate once, it stays on |
| Trigger | 40 | No — fires on its own condition |
| Meta | 29 | No — fires the skills socketed into it |

**Heralds, auras and other persistent buffs are activated once and then stay
on.** They reserve spirit and are turned on in the skill panel; they are never
pressed again. The same is true of trigger and meta gems — Cast on Critical,
Cast on Block, Spellslinger and the rest fire from their own condition, not from
your hand.

So a build listing 10 skill gems does not ask for 10 buttons.

### What builds actually run

Across the 63 endgame builds:

| Active skills | Builds |
|---|---|
| 1 | 4 |
| 2 | 5 |
| 3 | 9 |
| 4 | 12 |
| **5** | **19** |
| 6 | 7 |
| 7 | 5 |
| 9 | 1 |
| 12 | 1 |

Median **5** active skills, mean 4.5. **18 of 63 builds — 29% — run three
buttons or fewer.** Four run a single one.

Average endgame composition: **4.5 active, 3.4 persistent, 0.8 trigger, 1.1
meta.** Fewer than half the socketed gems are things you press.

### The button count never grows

This is the finding worth acting on. Medians per stage:

| Stage | Active | Persistent | Trigger | Meta | Total gems |
|---|---|---|---|---|---|
| Act 1 | 4 | 1 | 0 | 0 | 6 |
| Act 2 | 5 | 1 | 0 | 0 | 7 |
| Act 3 | 5 | 2 | 0 | 1 | 8 |
| Act 4 | 4 | 2 | 0 | 1 | 8 |
| Interludes | 5 | 2 | 0 | 0 | 9 |
| Early endgame | 5 | 3 | 1 | 1 | 10 |
| Endgame | 5 | 3 | 0 | 1 | 9 |
| Mid endgame | 4 | 3 | 2 | 1 | 9 |
| Late endgame | 4 | 4 | 2 | 1 | 11 |
| Uber | 4 | 3 | 1 | 1 | 10 |

**Active skills sit at 4–5 from Act 1 to uber endgame and never move.** Total gem
count rises from 6 to 11, and every gem added is a persistent buff, a trigger or
a meta gem.

A build does not get more complicated to play as it gets stronger. It gets more
automated. If anything, the strongest builds press *fewer* keys than mid-campaign
ones, because they have converted active skills into triggered ones.

### How builds reach a low button count

Two mechanisms, and the lowest-button builds in the dataset use one or the other.

**Stack persistent buffs.** Push nearly everything into spirit reservation and
leave one attack. *Supporting Fire Only Tact - One Button King* runs 9 persistent
buffs, 1 meta gem and **1** active skill. *Djinn Sorceress* does the same with 9
persistent buffs. This needs spirit, which caps at 100 from quests, so it is an
endgame pattern.

**Stack triggers and meta gems.** Let conditions fire your damage. *Ulf's Titan
1-BUTTON CASTER* runs **4 meta gems** (Spellslinger, Cast on Block, Cast on
Critical and one more) plus 1 trigger against only 2 active skills. *Ulf's
Earthshatter* runs 3 meta gems the same way.

The trigger and meta gems that appear most often:

| Builds | Gem | Kind |
|---|---|---|
| 23 | Wind Dancer | persistent + trigger |
| 13 | Blasphemy | meta (curse aura) |
| 13 | Cast on Critical | meta |
| 12 | Pounce | trigger |
| 8 | Ice-Tipped Arrows | trigger |
| 7 | Mirage Archer | trigger |
| 4 | Infernal Cry | trigger |
| 3 | Spellslinger | meta |
| 3 | Cast on Elemental Ailment | meta |

Meta gems run on Energy and reserve spirit — see
[04-skills-and-gems.md](04-skills-and-gems.md). A trigger-heavy build competes
for the same spirit a herald build wants, so the two approaches trade against
each other rather than stacking freely.

### What this means for advice

1. **Report active skills, not gem count.** "This build uses 9 gems" reads as
   nine keys. "Four buttons plus five things you turn on once" is the truth and
   is much less alarming.
2. **Target 4–5 active skills.** Above 6 is outside what most published builds
   ask for. Above 7 needs a reason.
3. **Adding power should add automation, not buttons.** When a build needs more
   damage or more defence, prefer a persistent buff, a trigger or a meta gem over
   another active skill.
4. **Ask about button preference** when it is not obvious. Some players want the
   one-button build and will accept less damage for it; it is a legitimate
   constraint, not a compromise.
5. **Spirit is the budget for both** low-button approaches. Check it before
   promising either.

## Why movement speed is on 57 of 63 boots

The count is in [11-gear-and-charms.md](11-gear-and-charms.md); the reason is
worth stating because it explains why the stat outranks damage in that slot.

**During the campaign**, most time is spent walking between objectives rather
than fighting. Movement speed shortens the whole campaign in a way no damage stat
does — killing faster saves seconds, moving faster saves hours.

**In the endgame**, mobility is defence. Every avoidance layer in
[01-defences.md](01-defences.md) starts with not being where the damage lands,
and the defences-basics guide lists movement speed as a primary avoidance layer
alongside evasion and block. Faster movement also means faster map clears, which
is the actual currency of endgame play.

So boots are not really a defensive slot competing with damage. They are the
mobility slot, and mobility pays into clear speed, campaign time and survival at
once. Treat missing movement speed on boots as a finding, not a preference.

The same reasoning explains **slow mitigation** appearing at 104 allocations in
the passive data. Being slowed removes the defence that movement speed provides,
so builds that value mobility pay to protect it.

## Sources

63 endgame `.build` files and 324 staged files. Skill classification from PoB's
own gem tags (`meta`, `trigger`, `persistent`) in `data.gems`. The reasoning on
movement speed and button preference is player context supplied by the user, not
derived from the files.
