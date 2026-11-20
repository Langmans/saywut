//! Speaker names from what is said: "ik ben Anna", "I'm Tom", "ich bin Eva".
//!
//! The phrases and name particles are data (one file per language, see
//! `saywut-naming`); this module only matches them. A match is a suggestion for the user to
//! confirm, never applied on its own: "this is Amsterdam calling" would match
//! "this is {name}" too.

use crate::transcript::{SpeakerId, Transcript};

/// Marks where the name goes in a phrase.
pub const NAME_SLOT: &str = "{name}";

/// Most capitalized words taken as a name; particles between them do not
/// count, so "Jan van der Berg" is two.
pub const MAX_NAME_WORDS: usize = 3;

/// A phrase that introduces the speaker's own name, as lowercase words
/// before the name slot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamePattern {
    lead: Vec<String>,
}

impl NamePattern {
    /// Parses a phrase like "ik ben {name}". The slot must come last and at
    /// least one word must come before it.
    pub fn parse(phrase: &str) -> Option<Self> {
        let (before, after) = phrase.split_once(NAME_SLOT)?;
        if !after.trim().is_empty() {
            return None;
        }
        let lead = words(before).map(|w| w.to_lowercase()).collect::<Vec<_>>();
        (!lead.is_empty()).then_some(Self { lead })
    }
}

/// A name heard in a segment, for a speaker who has none yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameSuggestion {
    /// The speaker who said it.
    pub speaker: SpeakerId,
    /// The name heard.
    pub name: String,
    /// Index of the segment it was heard in.
    pub segment: usize,
}

/// Words of `text`: runs of letters, digits, apostrophes and hyphens, so
/// "I'm" and "Jan-Willem" stay whole and punctuation falls away.
fn words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '\'' || c == '-'))
        .filter(|w| !w.is_empty())
}

/// The phrases and name particles of one or more languages.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NamingRules {
    /// Phrases that introduce a name.
    pub patterns: Vec<NamePattern>,
    /// Lowercase words allowed inside a name ("de" in "Anna de Vries").
    pub particles: Vec<String>,
}

/// The name right after `lead` in `words`, if any: capitalized words, with
/// lowercase particles allowed between them but not at the end.
fn name_after(words: &[&str], lead: &[String], particles: &[String]) -> Option<String> {
    let starts_upper = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    for start in 0..words.len().saturating_sub(lead.len()) {
        let matches = lead
            .iter()
            .zip(&words[start..])
            .all(|(l, w)| *l == w.to_lowercase());
        if !matches {
            continue;
        }
        let rest = &words[start + lead.len()..];
        let mut taken: Vec<&str> = Vec::new();
        let mut capitalized = 0;
        for w in rest {
            if starts_upper(w) && capitalized < MAX_NAME_WORDS {
                capitalized += 1;
                taken.push(w);
            } else if !taken.is_empty() && particles.iter().any(|p| p == w) {
                taken.push(w);
            } else {
                break;
            }
        }
        while taken.last().is_some_and(|w| !starts_upper(w)) {
            taken.pop();
        }
        if !taken.is_empty() {
            return Some(taken.join(" "));
        }
    }
    None
}

/// Suggestions for every unnamed speaker who introduces themselves, in
/// segment order, at most one per speaker (the first).
pub fn suggest_names(transcript: &Transcript, rules: &NamingRules) -> Vec<NameSuggestion> {
    let mut found: Vec<NameSuggestion> = Vec::new();
    for (index, seg) in transcript.segments.iter().enumerate() {
        let Some(speaker) = seg.speaker else { continue };
        let unnamed = transcript
            .speaker(speaker)
            .is_some_and(|s| s.name.is_none());
        if !unnamed || found.iter().any(|f| f.speaker == speaker) {
            continue;
        }
        let ws: Vec<&str> = words(&seg.text).collect();
        let name = rules
            .patterns
            .iter()
            .find_map(|p| name_after(&ws, &p.lead, &rules.particles));
        if let Some(name) = name {
            found.push(NameSuggestion {
                speaker,
                name,
                segment: index,
            });
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcript::{Segment, Speaker};

    /// Test rules: the given phrases with Dutch particles.
    fn patterns(phrases: &[&str]) -> NamingRules {
        NamingRules {
            patterns: phrases
                .iter()
                .map(|p| NamePattern::parse(p).unwrap())
                .collect(),
            particles: ["de", "van"].map(String::from).to_vec(),
        }
    }

    fn transcript(lines: &[(u32, &str)], named: &[(u32, &str)]) -> Transcript {
        let mut ids: Vec<u32> = lines.iter().map(|(s, _)| *s).collect();
        ids.sort_unstable();
        ids.dedup();
        Transcript {
            speakers: ids
                .into_iter()
                .map(|id| Speaker {
                    id: SpeakerId(id),
                    name: named
                        .iter()
                        .find(|(n, _)| *n == id)
                        .map(|(_, name)| (*name).to_owned()),
                })
                .collect(),
            segments: lines
                .iter()
                .map(|(s, text)| Segment {
                    start_ms: 0,
                    end_ms: 0,
                    speaker: Some(SpeakerId(*s)),
                    text: (*text).to_owned(),
                })
                .collect(),
            ..Transcript::default()
        }
    }

    fn names(found: &[NameSuggestion]) -> Vec<(u32, &str, usize)> {
        found
            .iter()
            .map(|f| (f.speaker.0, f.name.as_str(), f.segment))
            .collect()
    }

    #[test]
    fn parse_patterns() {
        assert_eq!(
            NamePattern::parse("Ik ben {name}"),
            Some(NamePattern {
                lead: vec!["ik".into(), "ben".into()]
            })
        );
        assert_eq!(NamePattern::parse("I'm {name}").unwrap().lead, vec!["i'm"]);
        assert_eq!(NamePattern::parse("{name} here"), None);
        assert_eq!(NamePattern::parse("{name}"), None);
        assert_eq!(NamePattern::parse("no slot"), None);
    }

    #[test]
    fn finds_names_in_each_language_style() {
        let p = patterns(&[
            "ik ben {name}",
            "mijn naam is {name}",
            "I'm {name}",
            "ich bin {name}",
        ]);
        let t = transcript(
            &[
                (0, "Goedemorgen, ik ben Anna de Vries."),
                (1, "Hi! I'm Tom, nice to meet you."),
                (2, "Und ich bin Jan-Willem"),
                (3, "Mijn naam is Eva van Dijk en ik ben er klaar voor."),
            ],
            &[],
        );
        assert_eq!(
            names(&suggest_names(&t, &p)),
            vec![
                (0, "Anna de Vries", 0),
                (1, "Tom", 1),
                (2, "Jan-Willem", 2),
                (3, "Eva van Dijk", 3)
            ]
        );
    }

    #[test]
    fn needs_a_capitalized_name_right_after_the_phrase() {
        let p = patterns(&["ik ben {name}"]);
        let t = transcript(
            &[
                (0, "Ik ben blij."),
                (0, "ik ben"),
                (0, "Ik ben de Vries"),
                (0, "Nu weet je: ik ben Piet van"),
            ],
            &[],
        );
        // "blij" and "de" are lowercase, the second line ends at the phrase;
        // the trailing particle "van" is dropped.
        assert_eq!(names(&suggest_names(&t, &p)), vec![(0, "Piet", 3)]);
    }

    #[test]
    fn caps_the_name_length() {
        let p = patterns(&["ik ben {name}"]);
        let t = transcript(&[(0, "ik ben Een Twee Drie Vier")], &[]);
        assert_eq!(names(&suggest_names(&t, &p)), vec![(0, "Een Twee Drie", 0)]);
    }

    #[test]
    fn first_suggestion_per_unnamed_speaker_only() {
        let p = patterns(&["ik ben {name}"]);
        let t = transcript(
            &[(0, "ik ben Anna"), (0, "ik ben Bea"), (1, "ik ben Cor")],
            &[(1, "Kees")],
        );
        assert_eq!(names(&suggest_names(&t, &p)), vec![(0, "Anna", 0)]);
    }

    #[test]
    fn ignores_segments_without_a_listed_speaker() {
        let p = patterns(&["ik ben {name}"]);
        let mut t = transcript(&[(0, "ik ben Anna")], &[]);
        t.segments[0].speaker = None;
        assert!(suggest_names(&t, &p).is_empty());
        t.segments[0].speaker = Some(SpeakerId(7));
        assert!(suggest_names(&t, &p).is_empty());
    }
}
