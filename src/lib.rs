use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{self, BufRead, BufReader};

#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub body: String,
}

impl Document {
    pub fn new(id: impl Into<String>, title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            body: body.into(),
        }
    }
}

pub fn load_jsonl(path: impl AsRef<std::path::Path>) -> io::Result<Vec<Document>> {
    BufReader::new(File::open(path)?)
        .lines()
        .enumerate()
        .filter(|(_, line)| line.as_ref().map_or(true, |line| !line.trim().is_empty()))
        .map(|(line_number, line)| {
            let line = line?;
            serde_json::from_str(&line).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid document on line {}: {}", line_number + 1, error),
                )
            })
        })
        .collect()
}

pub fn load_jsonl_dir(path: impl AsRef<std::path::Path>) -> io::Result<Vec<Document>> {
    let path = path.as_ref();
    let mut documents = Vec::new();
    let mut entries = std::fs::read_dir(path)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<io::Result<Vec<_>>>()?;
    entries.sort();

    for entry in entries {
        if entry.is_file() {
            if entry.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                continue;
            }
            documents.extend(load_jsonl(&entry)?);
        }
    }

    Ok(documents)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Token {
    pub term: String,
    pub position: usize,
}

pub fn analyze(text: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut position = 0;

    for character in text.chars() {
        if character.is_alphanumeric() {
            for lowercase in character.to_lowercase() {
                current.push(lowercase);
            }
        } else if !current.is_empty() {
            tokens.push(Token {
                term: std::mem::take(&mut current),
                position,
            });
            position += 1;
        }
    }

    if !current.is_empty() {
        tokens.push(Token {
            term: current,
            position,
        });
    }

    tokens
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Field {
    Title,
    Body,
}

impl Field {
    fn boost(self) -> f64 {
        match self {
            Self::Title => 2.0,
            Self::Body => 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Posting {
    pub document_id: usize,
    pub field: Field,
    pub positions: Vec<usize>,
}

impl Posting {
    fn term_frequency(&self) -> usize {
        self.positions.len()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Bm25Parameters {
    pub k1: f64,
    pub b: f64,
}

impl Default for Bm25Parameters {
    fn default() -> Self {
        Self { k1: 1.2, b: 0.75 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldScoreExplanation {
    pub field: Field,
    pub term_frequency: usize,
    pub document_frequency: usize,
    pub document_length: usize,
    pub average_field_length: f64,
    pub field_boost: f64,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TermScoreExplanation {
    pub term: String,
    pub fields: Vec<FieldScoreExplanation>,
    pub score: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreExplanation {
    pub query: String,
    pub terms: Vec<TermScoreExplanation>,
    pub phrase_matches: Vec<String>,
    pub score: f64,
    pub parameters: Bm25Parameters,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SearchResult {
    pub document: Document,
    pub score: f64,
    pub explanation: ScoreExplanation,
}

#[derive(Default)]
pub struct Index {
    documents: Vec<Document>,
    postings: HashMap<String, Vec<Posting>>,
    field_lengths: HashMap<(usize, Field), usize>,
    document_frequencies: HashMap<String, usize>,
}

impl Index {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_document(&mut self, document: Document) {
        let document_id = self.documents.len();
        self.index_field(document_id, Field::Title, &document.title);
        self.index_field(document_id, Field::Body, &document.body);
        self.documents.push(document);
    }

    pub fn len(&self) -> usize {
        self.documents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        let parameters = Bm25Parameters::default();
        let query_terms = parse_query_terms(query);
        if query_terms.is_empty() || limit == 0 {
            return Vec::new();
        }

        let phrase_terms: Vec<Vec<String>> = parse_phrases(query)
            .into_iter()
            .map(|phrase| {
                analyze(&phrase)
                    .into_iter()
                    .map(|token| token.term)
                    .collect()
            })
            .filter(|terms: &Vec<String>| !terms.is_empty())
            .collect();

        let mut candidates = HashSet::new();
        for term in &query_terms {
            if let Some(postings) = self.postings.get(term) {
                candidates.extend(postings.iter().map(|posting| posting.document_id));
            }
        }

        let mut results = candidates
            .into_iter()
            .filter_map(|document_id| {
                self.score_document(
                    document_id,
                    query,
                    &query_terms,
                    &phrase_terms,
                    parameters.clone(),
                )
            })
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.document.id.cmp(&right.document.id))
        });
        results.truncate(limit);
        results
    }

    fn index_field(&mut self, document_id: usize, field: Field, text: &str) {
        let tokens = analyze(text);
        self.field_lengths
            .insert((document_id, field), tokens.len());
        let mut term_positions: HashMap<String, Vec<usize>> = HashMap::new();
        for token in tokens {
            term_positions
                .entry(token.term)
                .or_default()
                .push(token.position);
        }

        for (term, positions) in term_positions {
            self.postings
                .entry(term.clone())
                .or_default()
                .push(Posting {
                    document_id,
                    field,
                    positions,
                });
        }

        let field_terms = self
            .postings
            .iter()
            .filter(|(_, postings)| {
                postings
                    .iter()
                    .any(|posting| posting.document_id == document_id && posting.field == field)
            })
            .count();
        let _ = field_terms;

        let mut seen_terms = HashSet::new();
        for token in analyze(text) {
            seen_terms.insert(token.term);
        }
        for term in seen_terms {
            let document_count = self
                .postings
                .get(&term)
                .map(|postings| {
                    postings
                        .iter()
                        .filter(|posting| posting.document_id == document_id)
                        .map(|posting| posting.document_id)
                        .collect::<HashSet<_>>()
                        .len()
                })
                .unwrap_or(0);
            self.document_frequencies.insert(term, document_count);
        }
    }

    fn score_document(
        &self,
        document_id: usize,
        query: &str,
        query_terms: &[String],
        phrases: &[Vec<String>],
        parameters: Bm25Parameters,
    ) -> Option<SearchResult> {
        let mut term_explanations = Vec::new();
        let mut total_score = 0.0;

        for term in query_terms {
            let Some(postings) = self.postings.get(term) else {
                continue;
            };
            let mut field_explanations = Vec::new();
            for field in [Field::Title, Field::Body] {
                let posting = postings
                    .iter()
                    .find(|posting| posting.document_id == document_id && posting.field == field);
                let Some(posting) = posting else { continue };
                let document_length = *self.field_lengths.get(&(document_id, field)).unwrap_or(&0);
                let average_field_length = self.average_field_length(field);
                let document_frequency = self.document_frequencies.get(term).copied().unwrap_or(0);
                let idf = ((self.documents.len() as f64 - document_frequency as f64 + 0.5)
                    / (document_frequency as f64 + 0.5)
                    + 1.0)
                    .ln();
                let frequency = posting.term_frequency() as f64;
                let length_norm = if average_field_length == 0.0 {
                    1.0
                } else {
                    1.0 - parameters.b
                        + parameters.b * document_length as f64 / average_field_length
                };
                let score = idf * (frequency * (parameters.k1 + 1.0))
                    / (frequency + parameters.k1 * length_norm)
                    * field.boost();
                total_score += score;
                field_explanations.push(FieldScoreExplanation {
                    field,
                    term_frequency: posting.term_frequency(),
                    document_frequency,
                    document_length,
                    average_field_length,
                    field_boost: field.boost(),
                    score,
                });
            }
            let score = field_explanations.iter().map(|field| field.score).sum();
            term_explanations.push(TermScoreExplanation {
                term: term.clone(),
                fields: field_explanations,
                score,
            });
        }

        let phrase_matches = phrases
            .iter()
            .filter(|phrase| self.document_contains_phrase(document_id, phrase))
            .map(|phrase| phrase.join(" "))
            .collect::<Vec<_>>();
        let phrase_bonus = phrase_matches.len() as f64;
        total_score += phrase_bonus;

        Some(SearchResult {
            document: self.documents[document_id].clone(),
            score: total_score,
            explanation: ScoreExplanation {
                query: query.to_string(),
                terms: term_explanations,
                phrase_matches,
                score: total_score,
                parameters,
            },
        })
    }

    fn average_field_length(&self, field: Field) -> f64 {
        if self.documents.is_empty() {
            return 0.0;
        }
        let total = (0..self.documents.len())
            .map(|document_id| *self.field_lengths.get(&(document_id, field)).unwrap_or(&0) as f64)
            .sum::<f64>();
        total / self.documents.len() as f64
    }

    fn document_contains_phrase(&self, document_id: usize, phrase: &[String]) -> bool {
        if phrase.is_empty() {
            return false;
        }
        [Field::Title, Field::Body].iter().any(|field| {
            let positions = phrase
                .iter()
                .enumerate()
                .filter_map(|(offset, term)| {
                    self.postings
                        .get(term)?
                        .iter()
                        .find(|posting| {
                            posting.document_id == document_id && posting.field == *field
                        })
                        .map(|posting| (offset, posting.positions.clone()))
                })
                .collect::<Vec<_>>();
            if positions.len() != phrase.len() {
                return false;
            }
            positions[0].1.iter().any(|start| {
                positions
                    .iter()
                    .enumerate()
                    .all(|(offset, (_, term_positions))| term_positions.contains(&(start + offset)))
            })
        })
    }
}

fn parse_query_terms(query: &str) -> Vec<String> {
    analyze(query).into_iter().map(|token| token.term).collect()
}

fn parse_phrases(query: &str) -> Vec<String> {
    let mut phrases = Vec::new();
    let mut remainder = query;
    while let Some(start) = remainder.find('"') {
        let after_start = &remainder[start + 1..];
        let Some(end) = after_start.find('"') else {
            break;
        };
        phrases.push(after_start[..end].to_string());
        remainder = &after_start[end + 1..];
    }
    phrases
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_index() -> Index {
        let mut index = Index::new();
        index.add_document(Document::new(
            "rust",
            "Rust search",
            "Rust makes systems programming productive.",
        ));
        index.add_document(Document::new(
            "bm25",
            "Ranking with BM25",
            "BM25 ranks documents using term frequency and document frequency.",
        ));
        index.add_document(Document::new(
            "query",
            "Query processing",
            "A search query can contain an exact phrase.",
        ));
        index
    }

    #[test]
    fn analyzer_is_unicode_aware_and_normalizes_case() {
        assert_eq!(
            analyze("Rust: Élan"),
            vec![
                Token {
                    term: "rust".to_string(),
                    position: 0
                },
                Token {
                    term: "élan".to_string(),
                    position: 1
                },
            ]
        );
    }

    #[test]
    fn title_matches_receive_a_boost() {
        let results = sample_index().search("rust", 10);
        assert_eq!(results[0].document.id, "rust");
        assert!(results[0].explanation.terms[0]
            .fields
            .iter()
            .any(|field| field.field == Field::Title && field.field_boost > 1.0));
    }

    #[test]
    fn phrase_queries_are_explained() {
        let results = sample_index().search("\"term frequency\"", 10);
        assert_eq!(results[0].document.id, "bm25");
        assert_eq!(
            results[0].explanation.phrase_matches,
            vec!["term frequency"]
        );
    }

    #[test]
    fn explanation_score_matches_result_score() {
        let results = sample_index().search("search", 10);
        assert!((results[0].score - results[0].explanation.score).abs() < f64::EPSILON);
        assert!(!results[0].explanation.terms.is_empty());
    }

    #[test]
    fn empty_queries_return_no_results() {
        assert!(sample_index().search("", 10).is_empty());
    }

    #[test]
    fn jsonl_documents_can_be_loaded_and_searched() {
        let path = std::env::temp_dir().join("local-search-test.jsonl");
        std::fs::write(
            &path,
            "{\"id\":\"one\",\"title\":\"Imported\",\"body\":\"Local document\"}\n",
        )
        .expect("write test document");

        let documents = load_jsonl(&path).expect("load test document");
        let mut index = Index::new();
        for document in documents {
            index.add_document(document);
        }

        assert_eq!(index.search("imported", 1)[0].document.id, "one");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn documents_can_be_loaded_from_a_directory() {
        let dir = std::env::temp_dir().join("local-search-docs-dir");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create document dir");
        std::fs::write(
            dir.join("first.jsonl"),
            "{\"id\":\"alpha\",\"title\":\"Alpha\",\"body\":\"Alpha body\"}\n",
        )
        .expect("write first file");
        std::fs::write(
            dir.join("second.jsonl"),
            "{\"id\":\"beta\",\"title\":\"Beta\",\"body\":\"Beta body\"}\n",
        )
        .expect("write second file");

        let documents = load_jsonl_dir(&dir).expect("load directory");
        let mut index = Index::new();
        for document in documents {
            index.add_document(document);
        }

        assert_eq!(index.len(), 2);
        assert_eq!(index.search("alpha", 1)[0].document.id, "alpha");
        let _ = std::fs::remove_dir_all(dir);
    }
}
