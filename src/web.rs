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
        return html_response(search_page("", &[], documents));
    }

    if let Some(query_string) = target.strip_prefix("/search?") {
        let query = parameter(query_string, "q").unwrap_or_default();
        let operator = Operator::And;
        let matching_documents = search(index, &query, operator);
        let ranked_results = rank(index, &query)
            .into_iter()
            .filter(|result| matching_documents.contains(&result.document_id))
            .collect::<Vec<_>>();

        return html_response(search_page(&query, &ranked_results, documents));
    }

    html_response_with_status(
        "404 Not Found",
        "<main><h1>404</h1><p>Page not found.</p></main>",
    )
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

fn search_page(
    query: &str,
    ranked_results: &[crate::ranking::ScoredDocument],
    documents: &[Document],
) -> String {
    let escaped_query = escape_html(query);
    let mut results = String::new();

    if !query.trim().is_empty() {
        results.push_str(&format!(
            "<p class=\"summary\">{} result(s)</p>",
            ranked_results.len()
        ));

        for (position, result) in ranked_results.iter().enumerate() {
            if let Some(document) = documents
                .iter()
                .find(|document| document.id == result.document_id)
            {
                results.push_str(&format!(
                    "<article><h2>{}. {}</h2><p class=\"score\">Score: {:.3}</p><p>{}</p></article>",
                    position + 1,
                    escape_html(&document.name),
                    result.score,
                    escape_html(&make_snippet(document, query, 30))
                ));
            }
        }
    }

    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Rust Search</title>
  <style>
    body {{ margin: 0; font-family: system-ui, sans-serif; background: #f7f8fa; color: #202124; }}
    main {{ max-width: 760px; margin: 48px auto; padding: 0 20px; }}
    h1 {{ margin-bottom: 24px; }}
    form {{ display: flex; gap: 8px; margin-bottom: 24px; }}
    input {{ flex: 1; min-width: 160px; padding: 12px; font-size: 16px; }}
    button {{ padding: 12px; font-size: 16px; }}
    article {{ background: white; border-radius: 8px; padding: 16px; margin: 12px 0; }}
    article h2 {{ margin: 0 0 8px; font-size: 18px; }}
    .score {{ color: #666; font-size: 13px; }}
  </style>
</head>
<body>
  <main>
    <h1>Rust Search</h1>
    <form action="/search" method="get">
      <input name="q" value="{escaped_query}" placeholder="Search the local corpus" autofocus>
      <button type="submit">Search</button>
    </form>
    {results}
  </main>
</body>
</html>"#,
    )
}

fn html_response(body: String) -> String {
    html_response_with_status("200 OK", &body)
}

fn html_response_with_status(status: &str, body: &str) -> String {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

#[cfg(test)]
mod tests {
    use super::{escape_html, parameter, search_page, url_decode};

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
        let page = search_page("", &[], &[]);

        assert!(!page.contains("<select"));
        assert!(page.contains("action=\"/search\""));
    }
}
