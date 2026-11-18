use std::collections::HashMap;

use crate::{index::InvertedIndex, tokenizer::tokenize};

const K1: f64 = 1.2;
const B: f64 = 0.75;

#[derive(Debug, PartialEq)]
pub struct ScoredDocument {
    pub document_id: usize,
    pub score: f64,
}

pub fn rank(index: &InvertedIndex, query: &str) -> Vec<ScoredDocument> {
    let terms = tokenize(query);
    let average_document_length = index.average_document_length();

    if terms.is_empty() || average_document_length == 0.0 {
        return Vec::new();
    }

    let mut scores = HashMap::new();

    for term in terms {
        let document_frequency = index.document_frequency(&term);
        let Some(documents) = index.documents_for(&term) else {
            continue;
        };

        let inverse_document_frequency =
            inverse_document_frequency(index.document_count(), document_frequency);

        for &document_id in documents {
            let term_frequency = index.term_frequency(document_id, &term) as f64;
            let document_length = index.document_length(document_id) as f64;
            let length_normalization = 1.0 - B + B * document_length / average_document_length;
            let term_score = inverse_document_frequency * (term_frequency * (K1 + 1.0))
                / (term_frequency + K1 * length_normalization);

            *scores.entry(document_id).or_insert(0.0) += term_score;
        }
    }

    let mut results = scores
        .into_iter()
        .map(|(document_id, score)| ScoredDocument { document_id, score })
        .collect::<Vec<_>>();

    results.sort_by(|left, right| {
        right
            .score
            .total_cmp(&left.score)
            .then_with(|| left.document_id.cmp(&right.document_id))
    });
    results
}

fn inverse_document_frequency(document_count: usize, document_frequency: usize) -> f64 {
    ((document_count as f64 - document_frequency as f64 + 0.5) / (document_frequency as f64 + 0.5)
        + 1.0)
        .ln()
}

#[cfg(test)]
mod tests {
    use super::rank;
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

    #[test]
    fn ranks_documents_that_match_the_query() {
        let index = InvertedIndex::from_documents(&[
            document(0, &["rust", "rust", "search"]),
            document(1, &["rust", "search", "index", "query"]),
            document(2, &["index"]),
        ]);

        let results = rank(&index, "rust search");

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].document_id, 0);
        assert!(results[0].score > results[1].score);
    }

    #[test]
    fn returns_no_results_for_unknown_or_empty_queries() {
        let index = InvertedIndex::from_documents(&[document(0, &["rust"])]);

        assert!(rank(&index, "missing").is_empty());
        assert!(rank(&index, " ").is_empty());
    }

    #[test]
    fn uses_document_id_to_break_equal_score_ties() {
        let index =
            InvertedIndex::from_documents(&[document(1, &["rust"]), document(0, &["rust"])]);

        let results = rank(&index, "rust");

        assert_eq!(
            results
                .iter()
                .map(|result| result.document_id)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }
}
