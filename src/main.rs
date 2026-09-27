use local_search::{load_jsonl, load_jsonl_dir, Index};
use serde::Serialize;
use std::sync::Arc;
use tiny_http::{Header, Method, Request, Response, Server, StatusCode};
use url::form_urlencoded;

const INDEX_HTML: &[u8] = include_bytes!("../web/index.html");
const STYLES_CSS: &[u8] = include_bytes!("../web/styles.css");
const APP_JS: &[u8] = include_bytes!("../web/app.js");

#[derive(Serialize)]
struct ApiResult {
    id: String,
    title: String,
    body: String,
    score: f64,
    phrase_matches: Vec<String>,
}

#[derive(Serialize)]
struct SearchResponse {
    query: String,
    total_results: usize,
    document_count: usize,
    results: Vec<ApiResult>,
}

#[derive(Serialize)]
struct IndexInfo {
    document_count: usize,
}

fn main() {
    let mut index = Index::new();
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "documents.jsonl".to_string());
    let documents = if std::path::Path::new(&path).is_dir() {
        load_jsonl_dir(&path).unwrap_or_else(|error| {
            eprintln!("Could not load documents from directory '{}': {}", path, error);
            std::process::exit(1);
        })
    } else {
        load_jsonl(&path).unwrap_or_else(|error| {
            eprintln!("Could not load documents from '{}': {}", path, error);
            std::process::exit(1);
        })
    };
    for document in documents {
        index.add_document(document);
    }

    let document_count = index.len();
    let index = Arc::new(index);
    let server = Server::http("127.0.0.1:8000").expect("start web server");
    println!("Local search web app ready with {document_count} documents");
    println!("Open http://127.0.0.1:8000");

    for request in server.incoming_requests() {
        handle_request(request, &index);
    }
}

fn handle_request(request: Request, index: &Index) {
    let url = request.url().to_string();
    let (path, query) = url.split_once('?').unwrap_or((&url, ""));

    match (request.method(), path) {
        (&Method::Get, "/") | (&Method::Get, "/index.html") => {
            send_bytes(request, INDEX_HTML, "text/html; charset=UTF-8")
        }
        (&Method::Get, "/styles.css") => send_bytes(request, STYLES_CSS, "text/css; charset=UTF-8"),
        (&Method::Get, "/app.js") => send_bytes(request, APP_JS, "text/javascript; charset=UTF-8"),
        (&Method::Get, "/api/info") => send_index_info(request, index),
        (&Method::Get, "/api/search") => send_search_results(request, index, query),
        _ => {
            let _ = request.respond(Response::empty(StatusCode(404)));
        }
    }
}

fn send_index_info(request: Request, index: &Index) {
    let info = IndexInfo {
        document_count: index.len(),
    };
    let body = serde_json::to_string(&info).expect("serialize index info");
    send_bytes(request, body.as_bytes(), "application/json; charset=UTF-8");
}

fn send_search_results(request: Request, index: &Index, query: &str) {
    let query = form_urlencoded::parse(query.as_bytes())
        .find(|(key, _)| key == "q")
        .map(|(_, value)| value.into_owned())
        .unwrap_or_default();
    let results = index
        .search(&query, 20)
        .into_iter()
        .map(|result| ApiResult {
            id: result.document.id,
            title: result.document.title,
            body: result.document.body,
            score: result.score,
            phrase_matches: result.explanation.phrase_matches,
        })
        .collect::<Vec<_>>();
    let response = SearchResponse {
        query: query.clone(),
        total_results: results.len(),
        document_count: index.len(),
        results,
    };
    let body = serde_json::to_string(&response).expect("serialize search results");
    send_bytes(request, body.as_bytes(), "application/json; charset=UTF-8");
}

fn send_bytes(request: Request, body: &[u8], content_type: &str) {
    let header = Header::from_bytes("Content-Type", content_type).expect("valid response header");
    let _ = request.respond(Response::from_data(body.to_vec()).with_header(header));
}
