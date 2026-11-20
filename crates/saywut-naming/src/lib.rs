//! The naming phrases of `data/naming/`, per speech language.

use saywut_core::naming::{NamePattern, NamingRules};
use serde::Deserialize;

include!(concat!(env!("OUT_DIR"), "/naming.rs"));

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    phrases: Vec<String>,
    #[serde(default)]
    particles: Vec<String>,
}

fn parse(source: &str) -> NamingRules {
    // The files are part of the build and checked by the tests below.
    let file: File = toml::from_str(source).expect("valid naming file");
    NamingRules {
        patterns: file
            .phrases
            .iter()
            .map(|p| NamePattern::parse(p).expect("valid naming phrase"))
            .collect(),
        particles: file.particles,
    }
}

/// Languages that have naming phrases (BCP-47 primary subtags), sorted.
pub fn languages() -> Vec<&'static str> {
    LANGUAGES.iter().map(|(tag, _)| *tag).collect()
}

/// Rules for a spoken language (`nl`, `nl-BE`, ...). For an unknown or
/// undetected language every language's rules are combined, since a
/// transcript without a language could be in any of them.
pub fn rules_for(language: Option<&str>) -> NamingRules {
    let primary = language.map(|tag| tag.split(['-', '_']).next().unwrap_or(tag).to_lowercase());
    let chosen: Vec<&str> = match LANGUAGES
        .iter()
        .find(|(tag, _)| Some(*tag) == primary.as_deref())
    {
        Some((_, source)) => vec![source],
        None => LANGUAGES.iter().map(|(_, source)| *source).collect(),
    };
    let mut rules = NamingRules::default();
    for source in chosen {
        let next = parse(source);
        rules.patterns.extend(next.patterns);
        for particle in next.particles {
            if !rules.particles.contains(&particle) {
                rules.particles.push(particle);
            }
        }
    }
    rules
}

#[cfg(test)]
mod tests {
    use super::*;
    use saywut_core::{
        naming::suggest_names,
        transcript::{Segment, Speaker, SpeakerId, Transcript},
    };

    fn said(text: &str) -> Transcript {
        Transcript {
            speakers: vec![Speaker {
                id: SpeakerId(0),
                name: None,
            }],
            segments: vec![Segment {
                start_ms: 0,
                end_ms: 0,
                speaker: Some(SpeakerId(0)),
                text: text.into(),
            }],
            ..Transcript::default()
        }
    }

    fn heard(language: Option<&str>, text: &str) -> Option<String> {
        suggest_names(&said(text), &rules_for(language))
            .first()
            .map(|s| s.name.clone())
    }

    #[test]
    fn ships_nl_en_de() {
        assert_eq!(languages(), vec!["de", "en", "nl"]);
    }

    #[test]
    fn every_file_parses() {
        for (tag, source) in LANGUAGES {
            let rules = parse(source);
            assert!(!rules.patterns.is_empty(), "{tag} has no phrases");
        }
    }

    #[test]
    fn per_language() {
        assert_eq!(
            heard(Some("nl"), "Ik ben Jan van der Berg."),
            Some("Jan van der Berg".into())
        );
        assert_eq!(
            heard(Some("en-GB"), "Hello, my name is Tom."),
            Some("Tom".into())
        );
        assert_eq!(
            heard(Some("de_CH"), "Ich heiße Eva von Berg"),
            Some("Eva von Berg".into())
        );
        // Dutch phrases are not used for English.
        assert_eq!(heard(Some("en"), "ik ben Anna"), None);
    }

    #[test]
    fn unknown_or_missing_language_uses_all() {
        assert_eq!(heard(None, "ik ben Anna"), Some("Anna".into()));
        assert_eq!(heard(Some("fr"), "I'm Tom"), Some("Tom".into()));
        // Particles shared by several languages are listed once.
        let all = rules_for(None);
        assert_eq!(all.particles.iter().filter(|p| *p == "de").count(), 1);
    }
}
