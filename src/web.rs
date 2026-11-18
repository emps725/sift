use std::{
    error::Error,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
};

use crate::{
    document::Document,
    index::InvertedIndex,
    query::{Operator, search},
    ranking::rank,
    snippet::make_snippet,
};

pub fn run_server(
    address: &str,
    documents: Vec<Document>,
    index: InvertedIndex,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(address)?;
    println!("Search frontend: http://{address}");

    for connection in listener.incoming() {
        let mut stream = connection?;
        if let Err(error) = handle_connection(&mut stream, &documents, &index) {
            eprintln!("Could not respond to request: {error}");
        }
    }

    Ok(())
}

fn handle_connection(
    stream: &mut TcpStream,
    documents: &[Document],
    index: &InvertedIndex,
) -> std::io::Result<()> {
    let mut request = [0; 8192];
    let bytes_read = stream.read(&mut request)?;
    let request = String::from_utf8_lossy(&request[..bytes_read]);
    let response = build_response(&request, documents, index);

    stream.write_all(response.as_bytes())?;
    stream.flush()
}

fn build_response(request: &str, documents: &[Document], index: &InvertedIndex) -> String {
    let target = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");

    if target == "/" {
        return asset_response(
            "200 OK",
            "text/html; charset=utf-8",
            include_str!("../web/index.html"),
        );
    }

    if target == "/styles.css" {
        return asset_response(
            "200 OK",
            "text/css; charset=utf-8",
            include_str!("../web/styles.css"),
        );
    }

    if target == "/app.js" {
        return asset_response(
            "200 OK",
            "application/javascript; charset=utf-8",
            include_str!("../web/app.js"),
        );
    }

    if let Some(query_string) = target.strip_prefix("/api/search?") {
        let query = parameter(query_string, "q").unwrap_or_default();
        let ranked_results = search_results(index, documents, &query);

        return asset_response("200 OK", "application/json; charset=utf-8", &ranked_results);
    }

    if let Some(query_string) = target.strip_prefix("/search?") {
        let query = parameter(query_string, "q").unwrap_or_default();
        let page = include_str!("../web/index.html").replace(
            "<body>",
            &format!(
                "<body data-initial-query=\"{}\">",
                escape_html_attribute(&query)
            ),
        );
        return asset_response("200 OK", "text/html; charset=utf-8", &page);
    }

    html_response_with_status(
        "404 Not Found",
        "<main><h1>404</h1><p>Page not found.</p></main>",
    )
}

fn search_results(index: &InvertedIndex, documents: &[Document], query: &str) -> String {
    let matching_documents = search(index, query, Operator::And);
    let ranked_results = rank(index, query)
        .into_iter()
        .filter(|result| matching_documents.contains(&result.document_id))
        .collect::<Vec<_>>();
    let mut results = String::from("{\"query\":");
    results.push_str(&format!("\"{}\",", json_escape(query)));
    results.push_str("\"results\":[");

    for (position, result) in ranked_results.iter().enumerate() {
        if let Some(document) = documents
            .iter()
            .find(|document| document.id == result.document_id)
        {
            if position > 0 {
                results.push(',');
            }

            results.push_str(&format!(
                "{{\"title\":\"{}\",\"name\":\"{}\",\"snippet\":\"{}\",\"score\":{:.6}}}",
                json_escape(&document.title),
                json_escape(&document.name),
                json_escape(&make_snippet(document, query, 30)),
                result.score
            ));
        }
    }

    results.push_str("]}");
    results
}

fn parameter(query: &str, name: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == name).then(|| url_decode(value))
    })
}

fn url_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut position = 0;

    while position < bytes.len() {
        match bytes[position] {
            b'+' => decoded.push(b' '),
            b'%' if position + 2 < bytes.len()
                && is_hex(bytes[position + 1])
                && is_hex(bytes[position + 2]) =>
            {
                decoded.push(hex_pair(bytes[position + 1], bytes[position + 2]));
                position += 2;
            }
            byte => decoded.push(byte),
        }
        position += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) || (b'A'..=b'F').contains(&byte)
}

fn hex_pair(first: u8, second: u8) -> u8 {
    fn hex_value(byte: u8) -> u8 {
        match byte {
            b'0'..=b'9' => byte - b'0',
            b'a'..=b'f' => byte - b'a' + 10,
            b'A'..=b'F' => byte - b'A' + 10,
            _ => 0,
        }
    }

    hex_value(first) * 16 + hex_value(second)
}

fn escape_html(value: &str) -> String {
    value
        .chars()
        .flat_map(|character| match character {
            '&' => "&amp;".chars().collect::<Vec<_>>(),
            '<' => "&lt;".chars().collect::<Vec<_>>(),
            '>' => "&gt;".chars().collect::<Vec<_>>(),
            '"' => "&quot;".chars().collect::<Vec<_>>(),
            '\'' => "&#39;".chars().collect::<Vec<_>>(),
            character => vec![character],
        })
        .collect()
}

fn escape_html_attribute(value: &str) -> String {
    escape_html(value)
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

fn asset_response(status: &str, content_type: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn html_response_with_status(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{escape_html, json_escape, parameter, search_results, url_decode};
    use crate::{document::Document, index::InvertedIndex};

    #[test]
    fn decodes_plus_and_percent_encoded_query_values() {
        assert_eq!(url_decode("rust+search"), "rust search");
        assert_eq!(url_decode("rust%20engine"), "rust engine");
    }

    #[test]
    fn reads_named_query_parameters() {
        assert_eq!(
            parameter("q=rust+search&operator=and", "q").as_deref(),
            Some("rust search")
        );
    }

    #[test]
    fn escapes_untrusted_html() {
        assert_eq!(
            escape_html("<script>&\"Rust\"</script>"),
            "&lt;script&gt;&amp;&quot;Rust&quot;&lt;/script&gt;"
        );
    }

    #[test]
    fn frontend_has_no_operator_selection() {
        let page = include_str!("../web/index.html");
        assert!(!page.contains("<select"));
        assert!(page.contains("id=\"search-form\""));
    }

    #[test]
    fn json_escape_handles_quotes_and_control_characters() {
        assert_eq!(json_escape("a \"quote\"\n"), "a \\\"quote\\\"\\n");
    }

    #[test]
    fn api_returns_serialized_results() {
        let documents = [Document {
            id: 0,
            name: "document.txt".to_string(),
            title: "Rust Search".to_string(),
            url: None,
            text: "rust search".to_string(),
            tokens: vec!["rust".to_string(), "search".to_string()],
        }];
        let index = InvertedIndex::from_documents(&documents);

        let response = search_results(&index, &documents, "rust");
        assert!(response.contains("\"title\":\"Rust Search\""));
        assert!(response.contains("\"results\":["));
    }
}
