pub fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| word.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::tokenize;

    #[test]
    fn tokenizes_words_and_normalizes_case() {
        assert_eq!(
            tokenize("Rust Search ENGINE"),
            vec!["rust", "search", "engine"]
        );
    }

    #[test]
    fn ignores_whitespace_between_words() {
        assert_eq!(
            tokenize("  local\ntext\tcorpus  "),
            vec!["local", "text", "corpus"]
        );
    }

    #[test]
    fn returns_no_tokens_for_empty_text() {
        assert!(tokenize("").is_empty());
    }
}
