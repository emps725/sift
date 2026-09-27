# Local Search

A deliberately small, local search engine written in Rust. Phase 1 has no crawler, network fetching, AI, embeddings, vector database, or external search library.

The current implementation includes:

- A custom positional inverted index
- Unicode-aware tokenization and normalization
- Title and body fields with title boosting
- BM25 scoring
- Phrase matching
- Structured score explanations
- A command-line search loop

## Run

Install Rust, then run:

```text
cargo test
cargo run
```

Then open `http://127.0.0.1:8000` in a browser. Search results are rendered in the web interface; the server also exposes `GET /api/search?q=rust` for JSON clients.

By default, `cargo run` loads documents from `documents.jsonl`. To search another JSONL file, pass its path:

```text
cargo run -- path\to\documents.jsonl
```

Each non-empty line must be a JSON object with `id`, `title`, and `body` string fields.

## Phase 1 boundary

Documents are local and loaded from JSONL. Crawling is intentionally deferred to Phase 2.
