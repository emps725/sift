mod document;
mod fetch;
mod index;
mod query;
mod ranking;
mod snippet;
mod tokenizer;
mod web;

use std::{env, error::Error, fs, io, path::Path};

use document::Document;
use fetch::fetch_document;
use index::InvertedIndex;
use query::{Operator, search};
use ranking::rank;
use snippet::make_snippet;
use web::run_server;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args().skip(1);
    let corpus_directory = arguments.next().unwrap_or_else(|| "corpus".to_string());

    if corpus_directory == "fetch" {
        let url = arguments.next().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "usage: search-engine fetch URL [QUERY...]",
            )
        })?;
        let query = arguments.collect::<Vec<_>>().join(" ");
        let document = fetch_document(0, &url)?;

        println!("Title: {}", document.title);
        println!("URL: {}", url);
        println!("Text tokens: {}", document.tokens.len());
        println!();
        println!("{}", make_snippet(&document, &query, 40));

        if !query.trim().is_empty() {
            let index = InvertedIndex::from_documents(std::slice::from_ref(&document));
            println!("\nBM25 results for \"{query}\":");
            for result in rank(&index, &query) {
                println!("  {} ({:.3})", document.title, result.score);
            }
        }

        return Ok(());
    }

    if corpus_directory == "serve" {
        let documents = load_corpus(Path::new("corpus"))?;
        let index = InvertedIndex::from_documents(&documents);
        return run_server("127.0.0.1:3000", documents, index);
    }

    let operator = arguments
        .next()
        .map(|value| parse_operator(&value))
        .transpose()?
        .unwrap_or(Operator::And);
    let query = arguments.collect::<Vec<_>>().join(" ");

    if query.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: search-engine [CORPUS_DIR] [and|or] QUERY...",
        )
        .into());
    }

    let documents = load_corpus(Path::new(&corpus_directory))?;
    let index = InvertedIndex::from_documents(&documents);
    let matching_documents = search(&index, &query, operator);
    let ranked_results = rank(&index, &query)
        .into_iter()
        .filter(|result| matching_documents.contains(&result.document_id))
        .collect::<Vec<_>>();

    println!("{operator:?} query: \"{query}\"");
    println!("{} result(s)", ranked_results.len());

    for (position, result) in ranked_results.iter().enumerate() {
        if let Some(document) = documents
            .iter()
            .find(|document| document.id == result.document_id)
        {
            println!(
                "\n{}. {} (score: {:.3})",
                position + 1,
                document.name,
                result.score
            );
            println!("   {}", make_snippet(document, &query, 12));
        }
    }

    Ok(())
}

fn parse_operator(value: &str) -> Result<Operator, io::Error> {
    match value.to_lowercase().as_str() {
        "and" => Ok(Operator::And),
        "or" => Ok(Operator::Or),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "operator must be either 'and' or 'or'",
        )),
    }
}

fn load_corpus(directory: &Path) -> Result<Vec<Document>, Box<dyn Error>> {
    let mut paths = fs::read_dir(directory)?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()?;

    paths.sort();

    paths
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
        .enumerate()
        .map(|(id, path)| Document::from_file(id, &path).map_err(Into::into))
        .collect()
}
