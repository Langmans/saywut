//! Word error rate, for comparing a transcript against a reference.
//!
//! Both texts are normalized first: lowercased, and every character that is
//! not a letter, digit or apostrophe becomes a word break. Punctuation and
//! casing differ between engines and are not what the budgets measure.

fn words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !(c.is_alphanumeric() || c == '\''))
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// (substitutions + deletions + insertions) / reference words. An empty
/// reference gives 0.0 for an empty hypothesis and 1.0 otherwise.
pub fn wer(reference: &str, hypothesis: &str) -> f64 {
    let (r, h) = (words(reference), words(hypothesis));
    if r.is_empty() {
        return if h.is_empty() { 0.0 } else { 1.0 };
    }
    // Levenshtein over words, one row at a time.
    let mut prev: Vec<usize> = (0..=h.len()).collect();
    for (i, rw) in r.iter().enumerate() {
        let mut row = vec![i + 1; h.len() + 1];
        for (j, hw) in h.iter().enumerate() {
            let substitute = prev[j] + usize::from(rw != hw);
            row[j + 1] = substitute.min(prev[j + 1] + 1).min(row[j] + 1);
        }
        prev = row;
    }
    prev[h.len()] as f64 / r.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_after_normalization() {
        assert_eq!(wer("Hello, World!", "hello world"), 0.0);
        assert_eq!(wer("ten o'clock.", "Ten o'clock"), 0.0);
    }

    #[test]
    fn counts_each_kind_of_error() {
        assert_eq!(wer("a b c d", "a x c d"), 0.25); // substitution
        assert_eq!(wer("a b c d", "a c d"), 0.25); // deletion
        assert_eq!(wer("a b c d", "a b b c d"), 0.25); // insertion
        assert_eq!(wer("a b", "c d e f"), 2.0); // can exceed 1
    }

    #[test]
    fn non_ascii_words() {
        assert_eq!(wer("Grüße aus Köln", "grüße aus koln"), 1.0 / 3.0);
    }

    #[test]
    fn empty_reference() {
        assert_eq!(wer("", " ... "), 0.0);
        assert_eq!(wer("", "extra"), 1.0);
    }
}
