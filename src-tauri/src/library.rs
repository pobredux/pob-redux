//! The game-knowledge library (`library/*.md` for PoE2 and `library/poe1/*.md`
//! at the repository root), compiled into the binary so the assistant can read a
//! topic on demand instead of carrying all of it in every prompt.

use crate::game::Game;

pub(crate) struct Topic {
    pub(crate) slug: &'static str,
    pub(crate) covers: &'static str,
    pub(crate) text: &'static str,
}

macro_rules! topic {
    ($slug:literal, $covers:literal, $file:literal) => {
        Topic { slug: $slug, covers: $covers, text: include_str!(concat!("../../library/", $file)) }
    };
}

pub(crate) const TOPICS: &[Topic] = &[
    topic!("progression-and-points", "Levels, the passive point budget, quest rewards, spirit", "00-progression-and-points.md"),
    topic!("defences", "Armour, evasion, energy shield, block, resistances, the damage calculation order", "01-defences.md"),
    topic!("damage", "Damage types, conversion, crit, accuracy, penetration, skill speed", "02-damage.md"),
    topic!("ailments", "Ignite, shock, freeze, chill, bleed, poison, stun, electrocute", "03-ailments.md"),
    topic!("skills-and-gems", "Gem tier to character level, support rules, meta gems, spirit costs", "04-skills-and-gems.md"),
    topic!("sustain-and-utility", "Leech, regeneration, recoup, flasks, charms, curses, marks", "05-sustain-and-utility.md"),
    topic!("tree-and-emotions", "Passive tree structure, attribute nodes, distilled emotion anoints", "06-tree-and-emotions.md"),
    topic!("advising-builds", "How to turn the library into a build recommendation; read first", "07-advising-builds.md"),
    topic!("keywords", "Short definitions: blind, exposure, withered, impale, rage, pin", "08-keywords.md"),
    topic!("buildcraft", "What 63 published builds do: tree composition, support depth, skill swaps", "09-buildcraft.md"),
    topic!("progression-curve", "Stage-by-stage targets: points, skills, supports, gear, uniques", "10-progression-curve.md"),
    topic!("gear-and-charms", "What goes in each gear slot, and why charms carry the unique budget", "11-gear-and-charms.md"),
    topic!("playstyle-and-buttons", "Button count as a design goal, automation via triggers, movement speed", "12-playstyle-and-buttons.md"),
    topic!("game-constants", "Hard numbers from PoB's engine: attribute bonuses, every cap, charges, thresholds", "13-game-constants.md"),
    topic!("keystones", "All 33 keystones with their downsides", "14-keystones.md"),
    topic!("runes-and-augments", "Runes, soul cores, augment sockets", "15-runes-and-augments.md"),
    topic!("evaluating-changes", "Guard rails for judging a change: requirements are a maximum not a sum, flat vs increased vs more damage, the sheet vs the build, dead stats, granted skills, a checklist", "16-evaluating-changes.md"),
];

pub(crate) const TOPICS_POE1: &[Topic] = &[
    topic!("progression-and-points", "Levels, the passive point budget, quest rewards, the bandit choice, ascendancy points", "poe1/00-progression-and-points.md"),
    topic!("defences", "Life, energy shield, armour, evasion, block, spell suppression, resistances, the damage order", "poe1/01-defences.md"),
    topic!("damage", "Damage types, conversion, crit, accuracy, penetration and exposure, skill speed", "poe1/02-damage.md"),
    topic!("ailments", "Ignite, bleed, poison, shock, chill, freeze, scorch, brittle, sap, avoidance", "poe1/03-ailments.md"),
    topic!("skills-and-gems", "Sockets and links, gem levels and quality, awakened, transfigured and Vaal gems, reservation", "poe1/04-skills-and-gems.md"),
    topic!("sustain-and-utility", "Leech, regeneration, recoup, flasks, curses, marks, guard skills, warcries", "poe1/05-sustain-and-utility.md"),
    topic!("tree-and-masteries", "Passive tree structure, masteries, cluster and timeless jewels, anointments, bloodlines", "poe1/06-tree-and-masteries.md"),
    topic!("advising-builds", "How to turn the library into a build recommendation; read first", "poe1/07-advising-builds.md"),
    topic!("keywords", "Short definitions: blind, maim, hinder, intimidate, withered, impale, exposure, onslaught", "poe1/08-keywords.md"),
    topic!("buildcraft", "What the builds in our PoE1 corpus do: skills, links, keystones, reservation, defences", "poe1/09-buildcraft.md"),
    topic!("progression-curve", "Stage-by-stage targets from the campaign to the endgame", "poe1/10-progression-curve.md"),
    topic!("gear-and-flasks", "What goes in each gear slot, sockets and links by slot, influence, flasks", "poe1/11-gear-and-flasks.md"),
    topic!("playstyle-and-buttons", "Button count as a design goal, automation through triggers and auras, movement speed", "poe1/12-playstyle-and-buttons.md"),
    topic!("game-constants", "Hard numbers from PoB's engine: attribute bonuses, every cap, charges, thresholds", "poe1/13-game-constants.md"),
    topic!("keystones", "Every keystone with its downside", "poe1/14-keystones.md"),
    topic!("pantheon-and-bandits", "Pantheon gods and their souls, the bandit choice", "poe1/15-pantheon-and-bandits.md"),
    topic!("evaluating-changes", "Guard rails for judging a change: requirements are a maximum not a sum, flat vs increased vs more damage, the sheet vs the build, reservation, dead stats, a checklist", "poe1/16-evaluating-changes.md"),
];

pub(crate) fn topics(game: Game) -> &'static [Topic] {
    match game {
        Game::Poe1 => TOPICS_POE1,
        Game::Poe2 => TOPICS,
    }
}

pub(crate) fn find(game: Game, slug: &str) -> Option<&'static Topic> {
    let want = slug.trim().to_ascii_lowercase();
    let all = topics(game);
    all.iter()
        .find(|t| t.slug == want)
        .or_else(|| all.iter().find(|t| t.slug.contains(&want) || t.covers.to_ascii_lowercase().contains(&want)))
}

/// One line per topic, for the tool description and the no-argument call.
pub(crate) fn index(game: Game) -> String {
    topics(game).iter().map(|t| format!("{}: {}", t.slug, t.covers)).collect::<Vec<_>>().join("; ")
}

#[cfg(test)]
mod tests {
    use super::TOPICS_POE1;

    #[test]
    fn poe1_topics_state_their_version_and_sources() {
        for t in TOPICS_POE1 {
            assert!(t.text.starts_with("# "), "{} has no title", t.slug);
            assert!(t.text.contains("Written against 3.29"), "{} does not say which patch it was written against", t.slug);
            assert!(t.text.contains("## Sources"), "{} has no Sources section", t.slug);
        }
    }
}
