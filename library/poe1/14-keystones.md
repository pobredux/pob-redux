# Keystones

Every keystone in PoE1 PoB's 3.29 tree data, with the tree text and what it costs. A keystone changes a rule, so
"which keystone does this build run" often explains a build faster than its notables do.

Written against 3.29 (PoB 2.67.2, tree 3_29).

## How many there are

PoB's `TreeData/3_29/tree.json` flags **57 nodes** as keystones:

| Group | Count | How a character gets one |
|---|---|---|
| On the passive tree | **48** | allocate it |
| Anoint only | 1 | Worship the Blightheart, anointed with three Prismatic Oils |
| Cluster jewel keystones | 8 | unique Small Cluster Jewels (the nodes have no tree position) |

PoB's timeless jewel data (`Data/TimelessJewelData/LegionPassives.lua`) adds **22 more** that only timeless
jewels create. poedb lists 49 keystone passives, 8 cluster keystones and 15 timeless keystones. Its timeless list
leaves out Heroic Tragedy and the Abyss eye jewels, which PoB includes.

`build_summary` returns `keystones`: every keystone the build has, whether allocated, granted by an item, or made
by a timeless jewel. The "Ladder" column counts how many of the 327 ladder characters in our PoE1 corpus have each
one, read through PoB after timeless conversion. 35 of the 327 have no keystone.

## Life, mana and energy shield

| Keystone | Tree text | What it changes, and the cost | Ladder |
|---|---|---|---|
| Chaos Inoculation | Maximum Life becomes 1, Immune to Chaos Damage | Energy shield is the whole pool. Life lines do nothing. PoB counts the build as always on full life. | 67 |
| Ghost Dance | Cannot Recover Energy Shield to above Evasion Rating. Every 2 seconds, gain a Ghost Shroud, up to a maximum of 3. When Hit, lose a Ghost Shroud to Recover Energy Shield equal to 3% of your Evasion Rating | Evasion restores energy shield when hit. Energy shield cannot rise above evasion. | 39 |
| Zealot's Oath | Life Regeneration is applied to Energy Shield instead | Life regeneration refills energy shield. Life gets no regeneration. | 36 |
| Eldritch Battery | Spend Energy Shield before Mana for Skill Mana Costs. Energy Shield protects Mana instead of Life. 50% less Energy Shield Recharge Rate | Energy shield pays for skills. It no longer protects life, and it recharges at half speed. | 36 |
| Divine Shield | Cannot Recover Energy Shield to above Armour. 3% of Physical Damage prevented from Hits Recently is Regenerated as Energy Shield per second | Armour feeds energy shield regeneration. Energy shield cannot rise above armour. | 30 |
| Blood Magic | Removes all mana. 10% more maximum Life. Skills Cost Life instead of Mana. Skills Reserve Life instead of Mana | Skills and auras use life. Mana lines do nothing. | 29 |
| Ghost Reaver | Leech Energy Shield instead of Life. Maximum total Energy Shield Recovery per second from Leech is doubled. Cannot Recharge Energy Shield | Leech moves to energy shield. Energy shield never recharges. | 28 |
| Mind Over Matter | 40% of Damage is taken from Mana before Life | Mana becomes a second pool against hits, so unreserved mana is defence. | 27 |
| Wicked Ward | Energy Shield Recharge is not interrupted by Damage if Recharge began Recently. 40% less Energy Shield Recharge Rate | Recharge keeps going through hits once it starts. Recharge is 40% slower. | 12 |
| The Agnostic | Removes all Energy Shield. While not on Full Life, Sacrifice 20% of Mana per Second to Recover that much Life | Mana heals life. The build has no energy shield. | 2 |
| Eternal Youth | 50% less Life Regeneration Rate. 50% less maximum Total Life Recovery per Second from Leech. Energy Shield Recharge instead applies to Life | Life recharges like energy shield. Energy shield stops recharging, and life regeneration and the leech cap are halved. | 2 |
| Vaal Pact | Life Leech from Melee Damage is Instant. Cannot Recover Life other than from Leech | Melee life leech lands at once. Life flasks, regeneration and recoup stop healing. | 1 |
| Solipsism | Intelligence provides no inherent bonus to Energy Shield. 2% reduced Duration of Elemental Ailments on you per 15 Intelligence | Intelligence shortens elemental ailments. It stops raising energy shield. | 0 |

## Avoidance and mitigation

| Keystone | Tree text | What it changes, and the cost | Ladder |
|---|---|---|---|
| Unwavering Stance | Cannot Evade enemy Attacks. Cannot be Stunned | Stun immunity. Evasion does nothing. | 45 |
| Versatile Combatant | -10% to maximum Chance to Block Attack Damage. -10% to maximum Chance to Block Spell Damage. +2% Chance to Block Spell Damage for each 1% Overcapped Chance to Block Attack Damage | Excess attack block becomes spell block. Both block caps drop to 65% unless raised. | 45 |
| Iron Reflexes | Converts all Evasion Rating to Armour. Dexterity provides no bonus to Evasion Rating | Evasion lines and evasion bases give armour. The build has no evasion. | 32 |
| Magebane | Dexterity provides no inherent bonus to Evasion Rating. +1% Chance to Suppress Spell Damage per 15 Dexterity | Dexterity gives spell suppression. It stops raising evasion. | 22 |
| Glancing Blows | Chance to Block Attack Damage is doubled. Chance to Block Spell Damage is doubled. You take 65% of Damage from Blocked Hits | Block reaches its cap with half the investment. Blocked hits still deal 65%. | 12 |
| Lethe Shade | Take 50% less Damage over Time if you've started taking Damage over Time in the past second. 100% more Duration of Ailments on you | Halves damage over time in its first second. Every ailment on you lasts twice as long. | 7 |
| Acrobatics | Modifiers to Chance to Suppress Spell Damage instead apply to Chance to Dodge Spell Hits at 50% of their value. Maximum Chance to Dodge Spell Hits is 75% | A dodged spell deals no damage. Suppression is gone, and each point of it becomes half a point of dodge. | 2 |
| Wind Dancer | 20% less Attack Damage taken if you haven't been Hit by an Attack Recently. 10% more chance to Evade Attacks if you have been Hit by an Attack Recently. 20% more Attack Damage taken if you have been Hit by an Attack Recently | Less attack damage while you avoid hits. After a hit, attacks deal 20% more. | 1 |
| Arrow Dancing | Evasion Rating is Doubled against Projectile Attacks. 25% less Evasion Rating against Melee Attacks | Strong against projectile attacks. Weaker against melee. | 0 |
| Imbalanced Guard | 100% chance to Defend with 200% of Armour. Maximum Damage Reduction for any Damage Type is 50% | Armour always counts double. Damage reduction stops at 50% instead of 90%. | 0 |

## Damage, accuracy and critical strikes

| Keystone | Tree text | What it changes, and the cost | Ladder |
|---|---|---|---|
| Resolute Technique | Your hits can't be Evaded. Never deal Critical Strikes | Accuracy stops mattering. No critical strikes. | 35 |
| Iron Will | Strength's Damage bonus applies to all Spell Damage as well | Strength scales spells. No cost beyond the points. | 28 |
| Pain Attunement | 30% more Spell Damage when on Low Life | Needs low life. PoB counts it when at least 50% of life is reserved. | 28 |
| Roiling Tempest | 25% more Maximum Lightning Damage. 50% less Minimum Lightning Damage. Cannot deal non-Lightning Damage | Raises the top of lightning rolls and lowers the bottom. Other damage types deal nothing. | 21 |
| Bitter Frost | Enemies Chilled by your Hits have Cold Damage taken increased by Chill Effect. Enemies in your Chilling Areas have Cold Damage taken increased by Chill Effect. Cannot deal non-Cold Damage | Chill also raises cold damage taken. Other damage types deal nothing. | 17 |
| Precise Technique | 40% more Attack Damage if Accuracy Rating is higher than Maximum Life. Never deal Critical Strikes | Needs accuracy above maximum life. No critical strikes. | 17 |
| Elemental Overload | Skills that have dealt a Critical Strike in the past 8 seconds deal 40% more Elemental Damage with Hits and Ailments. Your Critical Strikes do not deal extra Damage. Ailments never count as being from Critical Strikes | Any recent crit gives 40% more elemental damage. Critical strike multiplier does nothing. | 14 |
| Point Blank | Projectile Attack Hits deal up to 30% more Damage to targets at the start of their movement, dealing less Damage to targets as the projectile travels farther | Rewards close range. Far targets take less. | 12 |
| Elemental Equilibrium | Hits that deal Elemental Damage remove Exposure to those Elements and inflict Exposure to other Elements. Exposure inflicted this way applies -25% to Resistances | A hit of one element lowers the other two resistances by 25%. A hit of all three gains nothing. | 2 |
| Avatar of Fire | 50% of Physical, Cold and Lightning Damage Converted to Fire Damage. Deal no Non-Fire Damage | Half of other damage converts to fire. Chaos and the unconverted half deal nothing. | 2 |
| Perfect Agony | Damage over Time Multiplier for Ailments is equal to Critical Strike Multiplier. Critical Strikes do not deal extra Damage. Non-Critical Strikes cannot inflict Ailments | Critical strike multiplier scales ailments. Hits lose crit damage, and only crits apply ailments. | 1 |
| Iron Grip | Strength's Damage bonus applies to Projectile Attack Damage as well as Melee Damage | Strength scales projectile attacks. No cost beyond the points. | 0 |
| Crimson Dance | You can inflict Bleeding on an Enemy up to 8 times. Your Bleeding does not deal extra Damage while the Enemy is moving and cannot be Aggravated. 50% less Damage with Bleeding | Up to 8 bleeds stack. Each deals half, with no moving bonus and no aggravation. | 0 |
| Voracious Flame | You can inflict an additional Ignite on each Enemy. Base Ignite Duration is 1 second. 25% less Damage with Ignite. Cannot deal non-Fire Damage | Two ignites at once, each shorter and weaker. Other damage types deal nothing. | 0 |
| The Impaler | When your Hits Impale Enemies, also Impale other Enemies near them. Inflict 5 additional Impales on Enemies you Impale. For 5 seconds after you Impale Enemies, they cannot be Impaled again, and Impales cannot be Called from them | One impale becomes six and spreads. The target then cannot be impaled for 5 seconds. | 0 |

## Skills, minions, auras and curses

| Keystone | Tree text | What it changes, and the cost | Ladder |
|---|---|---|---|
| Ancestral Bond | You can't deal Damage with Skills yourself. +1 to maximum number of Summoned Totems | Two totems. Your own skills deal nothing. | 9 |
| Runebinder | -1 to maximum number of Summoned Totems. You can have an additional Brand Attached to an Enemy | Two brands on one enemy. The base totem limit drops to 0. | 8 |
| Bloodsoaked Blade | Tinctures inflict Weeping Wounds instead of Mana Burn. Effects that interact with Mana Burn interact with Weeping Wounds instead | The tincture drain moves from mana to life. | 2 |
| Minion Instability | Minions Explode when reduced to Low Life, dealing 33% of their Life as Fire Damage to surrounding Enemies | Minions explode at low life for fire damage. | 0 |
| Necromantic Aegis | All bonuses from an Equipped Shield apply to your Minions instead of you | Shield stats go to minions instead of you. | 0 |
| Supreme Ego | Auras from your Skills can only affect you. Aura Skills have 1% more Aura Effect per 2% of maximum Mana they Reserve. 40% more Mana Reservation of Aura Skills | Stronger auras on yourself. Allies and minions get none, and auras reserve 40% more. | 0 |
| Hex Master | Your Hexes have infinite Duration. 20% less Effect of your Curses | Hexes never expire. Every curse is 20% weaker. | 0 |
| Call to Arms | Your Warcries do not grant Buffs or Charges to You. 100% more Warcry Duration | Warcries last twice as long. They no longer buff you or give charges. | 0 |
| Conduit | Share Endurance, Frenzy and Power Charges with nearby party members | Party support. Does nothing solo. | 0 |
| Arsenal of Vengeance | Damaging Retaliation Skills become Usable every sixth Hit from Enemies instead | Retaliation skills unlock from taking six hits. | 0 |

## Keystones not on the tree

**Anoint only.** Worship the Blightheart: "Create Fungal Ground instead of Consecrated Ground". Three Prismatic
Oils on an amulet. The tree marks it `isBlighted`.

**Cluster jewel keystones.** Each comes from one unique Small Cluster Jewel in PoB's data. No ladder character in
the corpus runs one.

| Keystone | Jewel | Effect in short |
|---|---|---|
| Disciple of Kitava | Kitava's Teachings | Consumes a corpse each second to recover 5% life and mana; 10% more damage taken without one |
| Lone Messenger | Calamitous Visions | One herald only, with more buff effect and damage; aura skills are disabled |
| Nature's Patience | Natural Affinity | Grasping Vines while stationary: double damage chance and less damage taken |
| Secrets of Suffering | The Interrogation | No ignite, chill, freeze or shock; crits inflict Scorch, Brittle and Sapped |
| Kineticism | The Siege | Attack projectiles always bleed, maim and knock back; no pierce, fork or chain |
| Veteran's Awareness | The Front Line | Resistances, maximum resistances and physical reduction during a guard skill; 20% more damage taken after losing it |
| Hollow Palm Technique | One With Nothing | While unencumbered: counts as dual wielding, 40% more melee attack speed, added physical damage per 10 dexterity |
| Pitfighter | none | Tree text is a placeholder ("1% increased Fishing Line Strength"). No item grants it. |

**Timeless jewel keystones.** A timeless jewel replaces keystones in its radius with its conqueror's keystone.
These 22 exist only that way, or on the uniques named after the table.

| Keystone | Jewel: conqueror | Effect in short | Ladder |
|---|---|---|---|
| Supreme Ostentation | Elegant Hubris: Caspiro | Ignore attribute requirements; no inherent attribute bonuses | 42 |
| Inner Conviction | Militant Faith: Dominus | Power charges instead of frenzy charges; 3% more spell damage per power charge | 16 |
| Tempered by War | Lethal Pride: Rakiata | 50% of cold and lightning taken as fire; 50% less cold and lightning resistance | 16 |
| Corrupted Soul | Glorious Vanity: Doryani | 50% of non-chaos damage bypasses energy shield; 15% of life as extra energy shield | 13 |
| Divine Flesh | Glorious Vanity: Xibaqua | All damage bypasses energy shield; 50% of elemental taken as chaos; +5% maximum chaos resistance | 8 |
| The Traitor | Brutal Restraint: Balbala | Flasks gain 4 charges per empty flask slot every 5 seconds | 3 |
| Immortal Ambition | Glorious Vanity: Ahuana | Energy shield starts at 0, cannot recharge and decays; life leech continues into energy shield | 2 |
| Supreme Decadence | Elegant Hubris: Cadiro | Life flasks also recover energy shield; 30% less life recovery from flasks | 2 |
| Strength of Blood | Lethal Pride: Kaom | Non-instant life leech does not heal; physical reduction per leech rate | 1 |
| Second Sight | Brutal Restraint: Nasima | You are blind; 25% more melee crit chance while blinded | 1 |
| Transcendence | Militant Faith: Maxarius | Armour applies to elemental hits instead of physical; ‑15% maximum elemental resistances | 1 |
| Overwhelming Hate | Abyss murderous eye jewel | 20% less maximum life; life increases also raise maximum rage | 1 |
| Chainbreaker | Lethal Pride: Akoya | Mana regeneration becomes rage regeneration; skills cost 3 rage | 0 |
| Dance with Death | Brutal Restraint: Asenath | No helmet; your crit chance and crit damage are lucky; enemy crit damage against you is lucky | 0 |
| Power of Purpose | Militant Faith: Avarius | 80% of maximum mana converts to twice that much armour | 0 |
| Supreme Grandstanding | Elegant Hubris: Victario | Nearby allies and enemies share charges with you; enemies that hit you can gain charges | 0 |
| Black Scythe Training | Heroic Tragedy: Vorana | No energy shield; evade chance and physical reduction use 200% of ward instead of evasion and armour | 0 |
| Celestial Mathematics | Heroic Tragedy: Uhtred | With unbroken ward, the next attack breaks it for added cold damage equal to 25% of ward | 0 |
| The Unbreaking Circle | Heroic Tragedy: Medved | 40% less ward; ward has a 60% chance not to break | 0 |
| Weighted Exchange | Abyss searching eye jewel | 15% more damage from non-crits; 30% less from crits | 0 |
| Reconstructed Essence | Abyss hypnotic eye jewel | Life recoup also recovers mana; 50% less recoup | 0 |
| The Loyal Few | Abyss ghastly eye jewel | Minion bonuses per minion type; one spectre, skeleton and zombie at most | 0 |

The Abyss eye jewels drop from Zorath, Vile Assembled. PoB ties each keystone to an eye jewel type by its id.

Uniques that grant a timeless keystone directly: The Burden of Truth (Supreme Decadence), Soul Tether (Immortal
Ambition), Replica Soul Tether (Corrupted Soul), Voll's Protector (Inner Conviction) and Mahuxotl's Machination
(Corrupted Soul, Divine Flesh, Eternal Youth, Immortal Ambition, Vaal Pact). Skin of the Lords has one variant for
each tree keystone except Chaos Inoculation and Necromantic Aegis.

## keystoneRules in PoB Redux

`build_summary` also returns `keystoneRules`: plain rules for keystones that change which lines matter. They come
from `keystone.rules` in `crates/pob-engine/lua/bridge.lua`, which reads PoB's modifiers, so an item-granted
keystone counts too.

| Keystone | Rule fires in PoE1? | Ladder builds where it fired |
|---|---|---|
| Chaos Inoculation | **Yes**, from `output.ChaosInoculation` | 67 of 67 |
| Blood Magic | **Yes**, from mana being 0 | 29 of 29 |
| Mind Over Matter | **Yes**, from `sharedMindOverMatter` | 27 of 27, plus 1 from another source |
| Eldritch Battery | **Yes**, from the `EnergyShieldProtectsMana` flag | 36 of 36 |
| Iron Reflexes | **Yes**, from the `IronReflexes` flag | 32 of 32 |

What the two flag-based rules mean in practice:

- **Eldritch Battery.** Energy shield no longer protects life. Treat life as the only pool against hits unless
  Mind Over Matter is also taken.
- **Iron Reflexes.** Evasion lines and evasion bases become armour. An evasion base is not off-type.

No rule covers Zealot's Oath, Ghost Reaver, Eternal Youth, Vaal Pact or Ghost Dance. Read their rows above before
recommending recovery lines.

## Using keystones in advice

1. **Name the cost every time.** Every keystone above states one in its tree text.
2. **Check the pairings.** Chaos Inoculation with Zealot's Oath or Ghost Reaver fits. Vaal Pact with a plan that
   relies on life flasks does not.
3. **Look for converted keystones.** A timeless jewel can replace a tree keystone. `keystones` in `build_summary`
   shows the result after conversion, so it can differ from the node name on the tree.
4. **Price it with the tools.** `node_info` reads a keystone, `path_plan` costs the route, and `get_stats` before
   and after `alloc_node` shows what it changes.

## Sources

- PoB (PoE1): `src-tauri/resources/pob1/TreeData/3_29/tree.json` (57 keystone nodes and their `stats`),
  `Data/TimelessJewelData/LegionPassives.lua` (timeless keystones), `Data/ClusterJewels.lua` (cluster keystone
  list), `Data/Uniques/jewel.lua`, `belt.lua`, `body.lua`, `shield.lua`, `Data/Uniques/Special/Generated.lua`
  (item sources), `Modules/Data.lua` (`data.keystones`), `Modules/ModParser.lua`, `Modules/CalcPerform.lua`,
  `Modules/CalcDefence.lua`.
- PoB Redux: `crates/pob-engine/lua/bridge.lua` (`keystone.profile`, `keystone.names`, `keystone.rules`).
- Corpus: `S:\_projects_\_poe2_\corpus\poe1\index.json` (327 ladder characters without the `no-dps` flag) and
  `xml/*.xml`, each loaded through `pobctl eval` and read with `build_summary`.
- poedb: https://poedb.tw/us/Keystone (49 / 8 / 15 counts, timeless conqueror names),
  https://poedb.tw/us/Worship_the_Blightheart (Prismatic Oil anoint).
