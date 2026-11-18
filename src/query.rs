use std::collections::HashSet;

use crate::{index::InvertedIndex, tokenizer::tokenize};

#[derive(Debug, Clone, Copy)]
pub enum Operator {
    And,
    Or,
}

pub fn search(index: &InvertedIndex, query: &str, operator: Operator) -> HashSet<usize> {
    let terms = tokenize(query);

    match operator {
        Operator::And => documents_matching_all(index, &terms),
        Operator::Or => documents_matching_any(index, &terms),
    }
}

fn documents_matching_all(index: &InvertedIndex, terms: &[String]) -> HashSet<usize> {
    let Some(first_term) = terms.first() else {
        return HashSet::new();
    };

    let Some(first_documents) = index.documents_for(first_term) else {
        return HashSet::new();
    };

    terms
        .iter()
        .skip(1)
        .fold(first_documents.clone(), |matches, term| {
            let Some(documents) = index.documents_for(term) else {
                return HashSet::new();
            };

            matches.intersection(documents).copied().collect()
        })
}

fn documents_matching_any(index: &InvertedIndex, terms: &[String]) -> HashSet<usize> {
    terms.iter().fold(HashSet::new(), |mut matches, term| {
        if let Some(documents) = index.documents_for(term) {
            matches.extend(documents.iter().copied());
        }
        matches
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::{Operator, search};
    use crate::{document::Document, index::InvertedIndex};

    fn document(id: usize, tokens: &[&str]) -> Document {
        Document {
            id,
            name: format!("document-{id}.txt"),
            title: format!("document-{id}.txt"),
            url: None,
            text: tokens.join(" "),
            tokens: tokens.iter().map(|token| (*token).to_string()).collect(),
        }
    }

    fn index() -> InvertedIndex {
        InvertedIndex::from_documents(&[
            document(0, &["rust", "search"]),
            document(1, &["search", "index"]),
            document(2, &["rust", "index"]),
        ])
    }

    #[test]
    fn and_returns_documents_containing_every_term() {
        assert_eq!(
            search(&index(), "RUST SEARCH", Operator::And),
            HashSet::from([0])
        );
    }

    #[test]
    fn or_returns_documents_containing_at_least_one_term() {
        assert_eq!(
            search(&index(), "rust search", Operator::Or),
            HashSet::from([0, 1, 2])
        );
    }

    #[test]
    fn unknown_terms_are_handled_by_each_operator() {
        assert!(search(&index(), "rust missing", Operator::And).is_empty());
        assert_eq!(
            search(&index(), "rust missing", Operator::Or),
            HashSet::from([0, 2])
        );
    }

    #[test]
    fn empty_queries_return_no_documents() {
        assert!(search(&index(), "   ", Operator::And).is_empty());
        assert!(search(&index(), "   ", Operator::Or).is_empty());
    }
}
