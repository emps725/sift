use crate::{document::Document, tokenizer::tokenize};

pub fn make_snippet(document: &Document, query: &str, max_tokens: usize) -> String {
    if max_tokens == 0 || document.tokens.is_empty() {
        return String::new();
    }

    let query_terms = tokenize(query);
    let match_position = document
        .tokens
        .iter()
        .position(|token| query_terms.iter().any(|term| term == token))
        .unwrap_or(0);
    let start = match_position.saturating_sub(max_tokens / 2);
    let end = (start + max_tokens).min(document.tokens.len());
    let start = end.saturating_sub(max_tokens);

    let mut snippet = document.tokens[start..end].join(" ");
    if start > 0 {
        snippet = format!("... {snippet}");
    }
    if end < document.tokens.len() {
        snippet.push_str(" ...");
    }
    snippet
}

#[cfg(test)]
mod tests {
    use super::make_snippet;
    use crate::document::Document;

    fn document(tokens: &[&str]) -> Document {
        Document {
            id: 0,
            name: "document.txt".to_string(),
            title: "document.txt".to_string(),
            url: None,
            text: tokens.join(" "),
            tokens: tokens.iter().map(|token| (*token).to_string()).collect(),
        }
    }

    #[test]
    fn centers_snippet_around_a_query_match() {
        let document = document(&["one", "two", "three", "four", "five"]);

        assert_eq!(
            make_snippet(&document, "three", 3),
            "... two three four ..."
        );
    }

    #[test]
    fn returns_the_beginning_when_no_term_matches() {
        let document = document(&["one", "two", "three"]);

        assert_eq!(make_snippet(&document, "missing", 2), "one two ...");
    }

    #[test]
    fn handles_empty_documents_and_zero_length_snippets() {
        assert_eq!(make_snippet(&document(&[]), "query", 3), "");
        assert_eq!(make_snippet(&document(&["one"]), "query", 0), "");
    }
}
