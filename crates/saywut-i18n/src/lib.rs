//! UI strings from the Fluent files under `locales/`, which the Tauri
//! frontend will read as well.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

pub use fluent_bundle::FluentValue;

include!(concat!(env!("OUT_DIR"), "/locales.rs"));

/// Used when nothing requested matches, and the reference for key parity.
pub const FALLBACK: &str = "en";

/// Tags of the embedded UI locales, sorted.
pub fn available() -> Vec<&'static str> {
    LOCALES.iter().map(|(tag, _)| *tag).collect()
}

/// Picks the best embedded locale for a BCP-47 tag: an exact match, then the
/// same language with any region (`nl-BE` → `nl`), then [`FALLBACK`].
pub fn negotiate(requested: &str) -> &'static str {
    let Ok(wanted) = requested.replace('_', "-").parse::<LanguageIdentifier>() else {
        return FALLBACK;
    };
    let parsed = LOCALES
        .iter()
        .map(|(tag, _)| (*tag, tag.parse::<LanguageIdentifier>().unwrap()));
    let mut same_language = None;
    for (tag, id) in parsed {
        if id == wanted {
            return tag;
        }
        if id.language == wanted.language && same_language.is_none() {
            same_language = Some(tag);
        }
    }
    same_language.unwrap_or(FALLBACK)
}

pub struct Localizer {
    tag: &'static str,
    bundle: FluentBundle<FluentResource>,
}

impl Localizer {
    pub fn new(requested: &str) -> Self {
        let tag = negotiate(requested);
        let source = LOCALES.iter().find(|(t, _)| *t == tag).unwrap().1;
        let mut bundle = FluentBundle::new(vec![tag.parse().unwrap()]);
        // Unicode bidi isolation marks around placeables only confuse plain
        // terminal and file output; the frontend can isolate in markup.
        bundle.set_use_isolating(false);
        bundle
            .add_resource(FluentResource::try_new(source.to_owned()).expect("valid .ftl"))
            .expect("no duplicate messages");
        Self { tag, bundle }
    }

    pub fn tag(&self) -> &'static str {
        self.tag
    }

    /// The formatted message, or `id` itself when the message is missing, so
    /// a gap shows up as a visible key instead of an empty string.
    pub fn message(&self, id: &str, args: &[(&str, FluentValue)]) -> String {
        let Some(pattern) = self.bundle.get_message(id).and_then(|m| m.value()) else {
            return id.to_owned();
        };
        let mut fluent_args = FluentArgs::new();
        for (name, value) in args {
            fluent_args.set(*name, value.clone());
        }
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, Some(&fluent_args), &mut errors)
            .into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::{ast::Entry, parser};
    use std::collections::BTreeSet;

    fn message_ids(source: &str) -> BTreeSet<String> {
        let resource = parser::parse(source).expect("locale parses without errors");
        resource
            .body
            .into_iter()
            .filter_map(|entry| match entry {
                Entry::Message(m) => Some(m.id.name.to_owned()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn ships_en_nl_de() {
        assert_eq!(available(), vec!["de", "en", "nl"]);
    }

    #[test]
    fn every_locale_has_exactly_the_fallback_keys() {
        let reference = message_ids(LOCALES.iter().find(|(t, _)| *t == FALLBACK).unwrap().1);
        for (tag, source) in LOCALES {
            assert_eq!(message_ids(source), reference, "keys differ in {tag}");
        }
    }

    #[test]
    fn negotiation() {
        assert_eq!(negotiate("nl"), "nl");
        assert_eq!(negotiate("nl-BE"), "nl");
        assert_eq!(negotiate("de_CH"), "de");
        assert_eq!(negotiate("EN-us"), "en");
        assert_eq!(negotiate("fr"), FALLBACK);
        assert_eq!(negotiate("not a tag!"), FALLBACK);
    }

    #[test]
    fn formats_with_arguments() {
        let nl = Localizer::new("nl-NL");
        assert_eq!(nl.tag(), "nl");
        assert_eq!(nl.message("speaker-label", &[("n", 2.into())]), "Spreker 2");
        assert_eq!(
            Localizer::new("de").message("speaker-label", &[("n", 1.into())]),
            "Sprecher 1"
        );
    }

    #[test]
    fn missing_message_shows_its_id() {
        assert_eq!(
            Localizer::new("en").message("no-such-key", &[]),
            "no-such-key"
        );
    }
}
