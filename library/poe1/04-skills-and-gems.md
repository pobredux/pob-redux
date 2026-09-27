# Skills and gems

Sockets, links, gem levels, quality, the gem families, which supports can apply, and how reservation is worked
out. Numbers are PoB's unless a line says otherwise.

Written against 3.29 (PoB 2.67.2, tree 3_29), league Allflame.

## Sockets and links

A support gem changes only the active skills in the same linked group. The item decides how many gems a group can
hold. PoB's base data gives each slot a socket limit (`socketLimit`), and PoB warns when a slot holds more gems
than its item has sockets.

| Slot | Most sockets |
|---|---|
| Body armour | **6** |
| Two-handed weapons (axes, maces, swords, staves, bows) | **6** |
| Helmet, gloves, boots | **4** |
| One-handed weapons, shields | **3** |
| Rings, amulet, belt | none (the Unset Ring base has 1 socket) |

### Socket colours changed in 3.29

- Items now come with **white sockets** by default, and **any gem fits any socket**.
- A Chromatic Orb guarantees at least one red, green or blue socket.
- A gem in a socket of its own colour gets **+10% quality**. PoB models this (`MatchingSocketQualityBonus = 10`),
  on top of the gem's own quality.
- Gem colours in PoB's data: red (strength) 228 gems, green (dexterity) 291, blue (intelligence) 342, white 10
  (Portal, Detonate Mines, the Pacts and a few more).

Source: Maxroll's 3.29 reveal summary and league update. A guide older than 3.29 that says a gem needs a socket of
its colour is out of date.

### What the corpus links

In our PoE1 corpus the main skill sits in the body armour for 238 of 327 ladder characters, the main weapon for
36 and the helmet for 28. Of the 320 main-skill groups the corpus could match:

| Main-skill group | Characters |
|---|---|
| 6 gems | **256** |
| 5 supports on the main skill | **237** |
| 4 supports | 12 |
| 3 supports | 33 |
| 0 to 2 supports | 38 |

The other 19 six-gem groups hold two or more active skills, such as a trigger setup (Cyclone with a channelled
trigger) or a Vaal copy of the main skill. These counts use the main skill the corpus recorded, which is wrong for
some ladder characters (a movement skill picked as main). Counted by each build's largest link instead, as
[09-buildcraft.md](09-buildcraft.md) does, **246 of 324** run 5 supports.

`sanity_check` expects a 5-link from level 68 and a 6-link from level 85 when the main skill is alone in its link.

## Attribute requirements

Each gem has a weighting per attribute (100 means pure). PoB turns it into a requirement at the character level
that the gem's current level needs:

```
requirement = round((20 + 3 × (level requirement − 3)) × (weighting / 100)^0.9 × k)
k = 0.7 for skill gems, 0.5 for supports; anything below 14 becomes 0
```

| Gem at level 20 | Requirement |
|---|---|
| Fireball | 155 Int |
| Cyclone | 68 Str, 98 Dex |
| Added Fire Damage (support) | 111 Str |
| Enlighten level 3 | 73 Int |

- **Supports have their own requirement** in PoE1.
- The character needs the **highest single source**, never the sum. `list_gems` gives `req_str`, `req_dex` and
  `req_int` at the usable gem level; `build_summary.requirements` gives need, have and the item or gem that sets
  it. See [16-evaluating-changes.md](16-evaluating-changes.md).

## Gem levels

Every gem level has its own character level requirement (`levels[n].levelRequirement` in `Data/Skills`). All 287
base skill gems need **level 70 for gem level 20** and **72 for gem level 21**. The curves differ below that:

| Gem level | 1 | 5 | 10 | 15 | 20 | 21 |
|---|---|---|---|---|---|---|
| Skill gem that starts at 1 (34 gems) | 1 | 11 | 32 | 52 | 70 | 72 |
| Skill gem that starts at 12 (49) | 12 | 27 | 44 | 59 | 70 | 72 |
| Skill gem that starts at 28 (59, the largest group) | 28 | 40 | 50 | 60 | 70 | 72 |
| Support that starts at 8 (32) | 8 | 21 | 40 | 55 | 70 | 72 |

Other skill gems start at 4, 10, 16, 24, 34 or 38; other supports at 1, 4, 18, 24, 31 or 38.

- `list_gems` returns `req_level` (gem level 1) and `gem_level` (the highest level the character can use). It hides
  gems above the build's level unless `max_level` is 0.
- In our PoE1 corpus the main skill gem is level 21 for 178 of 326 ladder characters and level 20 for 130. 283 of
  327 socket at least one level 21 gem.

## Quality and corruption

- Normal quality goes to **20%**. Corruption can raise it to **23%**, and PoB's default gem quality setting stops
  there too.
- A Vaal Orb on a gem has four equally likely results (Maxroll): **±1 level** (at most 21), **±up to 10%
  quality** (at most 23%), **the Vaal version of the gem**, or nothing.
- PoB's "Corrupted Maximum" gem level setting is the natural maximum + 1, except for awakened gems.
- The socket colour bonus adds 10% on top, so a 20% gem in a matching socket counts as 30%.

In our PoE1 corpus, 192 of 327 ladder characters socket a gem above 20% quality.

## Gem families

| Family | In PoB | Max level | Level 1 needs | Notes |
|---|---|---|---|---|
| Skill gems | 287 | 20 | 1 to 38 | |
| Transfigured gems | 214 | 20 | 1 to 34 | Named "<skill> of <word>" |
| Vaal gems | 101 | 20 (Vaal Breach: 1) | 1 to 34 | Cost souls |
| Supports | 177 | 20 | 1 to 38 | |
| Exceptional supports | 48 | 3 | 72 (Empower, Enlighten, Enhance: 1) | Tagged Exceptional |
| Awakened supports | 38 | 5 (4 for Awakened Empower, Enlighten, Enhance) | 72; 80 at level 5 | 35 are legacy |

**Transfigured gems.** The Divine Font in the Labyrinth turns a skill gem into one of three transfigured choices,
keeping its experience, level and quality (Maxroll). Each is its own gem in `list_gems`. In our PoE1 corpus, 117 of
326 ladder main skills are transfigured.

**Vaal gems** grant two skills: the Vaal skill and its base skill. The Vaal skill costs souls (15 to 100 in PoB),
stores 1 or 2 uses, and blocks soul gain for a set time after use (Vaal Haste: 6 seconds). In our corpus 149 of
327 ladder characters socket one; Vaal Molten Shell (44), Vaal Haste (31) and Vaal Discipline (24) lead.

**Exceptional supports** include Empower, Enlighten, Enhance, Greater Multistrike and Greater Fork. 265 of 327
ladder characters use one: Enlighten 149, Empower 84, Enhance 82, Greater Multistrike 50.

**Awakened supports.** PoB 2.67.2 tags 35 of the 38 as legacy gems. Its gem list hides them, and so do
`list_gems` and `list_valid_supports`. poedb still lists them with no legacy mark. None of the 327 ladder
characters uses one. The three that remain, Awakened Empower, Enlighten and Enhance, appear on 28.

## Which supports can apply

`list_valid_supports` runs PoB's own check (`calcLib.canGrantedEffectSupportActiveSkill`) against the group's
active skill:

1. The skill's types, including types other supports add, must match the support's required types and none of
   its excluded types.
2. A weapon-restricted support needs a skill that uses that weapon type.
3. Some skills cannot be supported, and some supports work only on skills from gems.
4. Trigger supports apply only to skills the player uses.

The bridge then drops legacy gems and supports whose gem level 1 needs a higher character level. `sort_by_dps`
adds each support to the group, reruns PoB and returns `dps_delta`. Use that to choose supports on a damage
skill.

## Reservation

Auras, heralds, stances and some buffs reserve part of the mana pool, or life with Blood Magic or Arrogance.

| Base reservation (gem level 20) | Skills |
|---|---|
| 50% | Anger, Wrath, Hatred, Grace, Determination, Haste, Malevolence, Zealotry, Pride, Purity of Elements |
| 35% | Discipline, Purity of Fire, Ice and Lightning, Summon Skitterbots |
| 25% | Heralds, Flesh and Stone, Arctic Armour, Tempest Shield |
| Flat mana | Clarity 279, Vitality 233, Precision 186 (lower at lower gem levels) |

### How PoB computes it

```
reserved = base × support multipliers × (1 + increased/reduced reservation) × more/less reservation
         ÷ (1 + increased reservation efficiency) ÷ more reservation efficiency
```

- A support's "Cost & Reservation Multiplier" is 100% plus its `manaMultiplier`. Enlighten level 3 is 92%.
  Arrogance, which reserves life instead of mana, is 220% at level 1 and 201% at level 20.
- Reduced reservation multiplies. Efficiency divides. On a 50% aura, 12% reduced reservation gives 44%; 12%
  increased efficiency gives 44.64%. Efficiency cannot go below −100%.
- PoB truncates each stage and rounds the result.
- Reserving 50% or more of life puts the character on low life (see
  [13-game-constants.md](13-game-constants.md)).

`build_summary` reports `manaReserved`, `manaReservedPercent`, `manaUnreserved` and the life versions.
`sanity_check` flags a main skill PoB counts as unusable because reservation leaves too little to pay for it.

In our PoE1 corpus, ladder characters run a median of 3 auras and heralds. The most common are Flesh and Stone
(115 of 327), Precision (95), Vitality (69), Herald of Purity (57) and Purity of Elements (55). 139 take the Mana
Mastery "12% increased Mana Reservation Efficiency of Skills".

## Checklist before recommending gems

1. `set_level`, then `set_gem_levels` on a levelling build.
2. `list_gems` for names and levels. Never name a gem from memory.
3. `list_valid_supports` with `sort_by_dps` for the damage skill.
4. Count the links: the gems must fit the item's sockets.
5. Read `build_summary.requirements` after the change.
6. Read `manaUnreserved` before adding an aura or herald.

## Sources

- PoB PoE1 (`src-tauri/resources/pob1`): `Data/Gems.lua`, `Data/Skills/*.lua` (level requirements, reservation,
  `manaMultiplier`, soul costs), `Data/Bases/*.lua` (`socketLimit`), `Modules/CalcTools.lua`
  (`getGemStatRequirement`, `canGrantedEffectSupportActiveSkill`), `Modules/CalcPerform.lua` (requirements,
  reservation), `Modules/CalcSetup.lua` and `Modules/Data.lua` (`MatchingSocketQualityBonus`),
  `Classes/SkillsTab.lua` (quality cap 23, gem level options), `Classes/GemTooltip.lua` (cost and reservation
  multiplier), `changelog.txt` (2.67.0).
- PathOfBuilding git: commit `08bbeb2bd` (legacy gem tags).
- PoB Redux: `crates/pob-engine/lua/bridge.lua` (`list_gems`, `list_valid_supports`, `build_summary`,
  `sanity_check`), `src-tauri/src/tools.rs`.
- Our PoE1 corpus: `corpus/poe1/stages.jsonl`, `index.json` and `xml/*.xml` (main-skill links, gem levels and
  quality, gem families, auras on 327 ladder characters).
- Maxroll: <https://maxroll.gg/poe/news/3-29-curse-of-the-allflame-reveal-summary>,
  <https://maxroll.gg/poe/news/poe-3-29-curse-of-the-allflame-league-branch-update>,
  <https://maxroll.gg/poe/resources/corruption>, <https://maxroll.gg/poe/getting-started/labyrinth-guide>,
  <https://maxroll.gg/poe/getting-started/getting-stronger>
- poedb: <https://poedb.tw/us/Awakened_Added_Fire_Damage_Support>
