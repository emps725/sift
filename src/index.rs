use std::collections::{HashMap, HashSet};

use crate::document::Document;

#[derive(Debug, Default)]
pub struct InvertedIndex {
    postings: HashMap<String, HashSet<usize>>,
    term_frequencies: HashMap<usize, HashMap<String, usize>>,
    document_lengths: HashMap<usize, usize>,
    document_count: usize,
    total_token_count: usize,
}

impl InvertedIndex {
    pub fn from_documents(documents: &[Document]) -> Self {
        let mut index = Self {
            document_count: documents.len(),
            ..Self::default()
        };

        for document in documents {
            index
                .document_lengths
                .insert(document.id, document.tokens.len());
            index.total_token_count += document.tokens.len();

            for token in &document.tokens {
                index
                    .postings
                    .entry(token.clone())
                    .or_default()
                    .insert(document.id);
                *index
                    .term_frequencies
                    .entry(document.id)
                    .or_default()
                    .entry(token.clone())
                    .or_default() += 1;
            }
        }

        index
    }

    pub fn documents_for(&self, term: &str) -> Option<&HashSet<usize>> {
        self.postings.get(term)
    }

    pub fn document_count(&self) -> usize {
        self.document_count
    }

    pub fn document_frequency(&self, term: &str) -> usize {
        self.documents_for(term).map_or(0, HashSet::len)
    }

    pub fn term_frequency(&self, document_id: usize, term: &str) -> usize {
        self.term_frequencies
            .get(&document_id)
            .and_then(|frequencies| frequencies.get(term))
            .copied()
            .unwrap_or(0)
    }

    pub fn document_length(&self, document_id: usize) -> usize {
        self.document_lengths
            .get(&document_id)
            .copied()
            .unwrap_or(0)
    }

    pub fn average_document_length(&self) -> f64 {
        if self.document_count == 0 {
            0.0
        } else {
            self.total_token_count as f64 / self.document_count as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::InvertedIndex;
    use crate::document::Document;

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

    #[test]
    fn finds_documents_containing_a_term() {
        let documents = [
            document(0, &["rust", "search"]),
            document(1, &["search", "index"]),
        ];
        let index = InvertedIndex::from_documents(&documents);

        let matching_documents = index.documents_for("search").unwrap();

        assert_eq!(matching_documents.len(), 2);
        assert!(matching_documents.contains(&0));
        assert!(matching_documents.contains(&1));
    }

    #[test]
    fn stores_each_document_once_for_repeated_terms() {
        let documents = [document(0, &["rust", "rust", "rust"])];
        let index = InvertedIndex::from_documents(&documents);

        assert_eq!(index.documents_for("rust").unwrap().len(), 1);
    }

    #[test]
    fn returns_none_for_unknown_terms() {
        let documents = [document(0, &["rust"])];
        let index = InvertedIndex::from_documents(&documents);

        assert!(index.documents_for("search").is_none());
    }

    #[test]
    fn stores_term_frequencies_and_document_lengths() {
        let documents = [
            document(0, &["rust", "rust", "search"]),
            document(1, &["search"]),
        ];
        let index = InvertedIndex::from_documents(&documents);

        assert_eq!(index.term_frequency(0, "rust"), 2);
        assert_eq!(index.term_frequency(1, "rust"), 0);
        assert_eq!(index.document_length(0), 3);
        assert_eq!(index.average_document_length(), 2.0);
    }
}
