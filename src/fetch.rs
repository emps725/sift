use std::time::Duration;

use scraper::{Html, Selector};

use crate::{document::Document, tokenizer::tokenize};

pub fn fetch_document(id: usize, url: &str) -> Result<Document, Box<dyn std::error::Error>> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("Rust-Search-Education/0.1")
        .build()?;
    let html = client.get(url).send()?.error_for_status()?.text()?;

    Ok(extract_html(id, url, &html))
}

pub fn extract_html(id: usize, url: &str, html: &str) -> Document {
    let document = Html::parse_document(html);
    let title = extract_title(&document).unwrap_or_else(|| url.to_string());
    let text = extract_text(&document);
    let name = title.clone();
    let searchable_text = format!("{title} {text}");

    Document {
        id,
        name,
        title,
        url: Some(url.to_string()),
        text: text.clone(),
        tokens: tokenize(&searchable_text),
    }
}

fn extract_title(document: &Html) -> Option<String> {
    let selector = Selector::parse("title").ok()?;
    let title = document
        .select(&selector)
        .next()?
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    (!title.is_empty()).then_some(title)
}

fn extract_text(document: &Html) -> String {
    let Ok(selector) = Selector::parse("body") else {
        return String::new();
    };

    document
        .select(&selector)
        .next()
        .map(|body| {
            body.text()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::extract_html;

    #[test]
    fn extracts_title_and_text() {
        let document = extract_html(
            0,
            "https://example.com",
            "<html><head><title>Example Page</title></head><body><p>Hello Rust search.</p></body></html>",
        );

        assert_eq!(document.title, "Example Page");
        assert_eq!(document.url.as_deref(), Some("https://example.com"));
        assert_eq!(document.text, "Hello Rust search.");
        assert_eq!(
            document.tokens,
            vec!["example", "page", "hello", "rust", "search."]
        );
    }

    #[test]
    fn uses_url_as_title_when_title_is_missing() {
        let document = extract_html(
            0,
            "https://example.com/article",
            "<html><body><p>Local text</p></body></html>",
        );

        assert_eq!(document.title, "https://example.com/article");
    }

    #[test]
    fn handles_empty_html() {
        let document = extract_html(0, "https://example.com", "<html></html>");

        assert_eq!(document.title, "https://example.com");
        assert!(document.text.is_empty());
        assert!(!document.tokens.is_empty());
    }
}
