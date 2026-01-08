use crate::error::{MdwnError, Result};
use scraper::{Html, Selector};
use serde_json::Value;
use url::Url;

/// Result of parsing HTML for markdown links
pub enum HtmlParseResult {
    /// Found a markdown link in the HTML
    MarkdownLink(Url),
    /// No markdown link found, need to convert HTML to markdown
    NeedsConversion,
}

/// Parse HTML to find <link rel="alternate" type="text/markdown"> tag
pub fn parse_html_for_markdown_link(html: &str, base_url: &Url) -> Result<HtmlParseResult> {
    let document = Html::parse_document(html);

    // Check for <base> tag first
    let base_selector = Selector::parse("base[href]").expect("valid CSS selector");
    let effective_base = document
        .select(&base_selector)
        .next()
        .and_then(|el| el.value().attr("href"))
        .and_then(|href| base_url.join(href).ok())
        .unwrap_or_else(|| base_url.clone());

    // Look for markdown link
    // Match: <link rel="alternate" type="text/markdown"> or <link rel="alternate" type="text/x-markdown">
    let link_selector = Selector::parse("link[rel='alternate']").expect("valid CSS selector");

    for link in document.select(&link_selector) {
        let link_type = link.value().attr("type").unwrap_or("");

        // Check for markdown MIME types (case-insensitive)
        let link_type_lower = link_type.to_lowercase();
        if (link_type_lower == "text/markdown" || link_type_lower == "text/x-markdown")
            && let Some(href) = link.value().attr("href") {
                // Resolve relative URL against base
                let markdown_url = effective_base
                    .join(href)
                    .map_err(|e| MdwnError::ParseError(format!("Invalid markdown link URL: {}", e)))?;

                return Ok(HtmlParseResult::MarkdownLink(markdown_url));
            }
    }

    // No markdown link found
    Ok(HtmlParseResult::NeedsConversion)
}

/// Result of parsing JSON for markdown content
pub enum JsonParseResult {
    /// Found a URL to fetch markdown from
    MarkdownUrl(Url),
    /// Found inline markdown content
    MarkdownContent(String),
    /// No markdown field found
    NotFound,
}

/// Parse JSON to find mdwn or markdown field (top-level only)
pub fn parse_json_for_markdown(json_str: &str, base_url: &Url) -> Result<JsonParseResult> {
    let value: Value =
        serde_json::from_str(json_str).map_err(|e| MdwnError::ParseError(format!("Invalid JSON: {}", e)))?;

    // Only check top-level object
    let obj = match &value {
        Value::Object(obj) => obj,
        _ => return Ok(JsonParseResult::NotFound),
    };

    // Check for "mdwn" field first, then "markdown"
    let md_value = obj.get("mdwn").or_else(|| obj.get("markdown"));

    match md_value {
        Some(Value::String(s)) => {
            // Check if it's a URL or inline content
            if s.starts_with("http://") || s.starts_with("https://") {
                // Absolute URL
                let url = Url::parse(s)
                    .map_err(|e| MdwnError::ParseError(format!("Invalid URL in mdwn field: {}", e)))?;
                Ok(JsonParseResult::MarkdownUrl(url))
            } else if s.starts_with('/') {
                // Relative URL
                let url = base_url
                    .join(s)
                    .map_err(|e| MdwnError::ParseError(format!("Invalid relative URL: {}", e)))?;
                Ok(JsonParseResult::MarkdownUrl(url))
            } else {
                // Inline markdown content
                Ok(JsonParseResult::MarkdownContent(s.clone()))
            }
        }
        _ => Ok(JsonParseResult::NotFound),
    }
}

/// Determine the content type category from MIME type
#[derive(Debug, PartialEq)]
pub enum ContentCategory {
    Markdown,
    PlainText,
    Html,
    Json,
    Unsupported(String),
}

/// Categorize a MIME type
pub fn categorize_content_type(mime_type: Option<&str>) -> ContentCategory {
    match mime_type {
        Some(mt) => {
            let mt_lower = mt.to_lowercase();
            if mt_lower == "text/markdown" || mt_lower == "text/x-markdown" {
                ContentCategory::Markdown
            } else if mt_lower == "text/plain" {
                ContentCategory::PlainText
            } else if mt_lower == "text/html" || mt_lower == "application/xhtml+xml" {
                ContentCategory::Html
            } else if mt_lower == "application/json" || mt_lower.ends_with("+json") {
                ContentCategory::Json
            } else {
                ContentCategory::Unsupported(mt.to_string())
            }
        }
        None => ContentCategory::Unsupported("unknown".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_html_with_markdown_link() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <link rel="alternate" type="text/markdown" href="/article.md">
            </head>
            <body></body>
            </html>
        "#;
        let base = Url::parse("https://example.com/page").unwrap();
        let result = parse_html_for_markdown_link(html, &base).unwrap();

        match result {
            HtmlParseResult::MarkdownLink(url) => {
                assert_eq!(url.as_str(), "https://example.com/article.md");
            }
            _ => panic!("Expected MarkdownLink"),
        }
    }

    #[test]
    fn test_parse_html_with_x_markdown() {
        let html = r#"
            <link rel="alternate" type="text/x-markdown" href="content.md">
        "#;
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_html_for_markdown_link(html, &base).unwrap();

        match result {
            HtmlParseResult::MarkdownLink(url) => {
                assert_eq!(url.as_str(), "https://example.com/content.md");
            }
            _ => panic!("Expected MarkdownLink"),
        }
    }

    #[test]
    fn test_parse_html_no_markdown_link() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <link rel="stylesheet" href="style.css">
            </head>
            <body><p>Hello</p></body>
            </html>
        "#;
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_html_for_markdown_link(html, &base).unwrap();

        assert!(matches!(result, HtmlParseResult::NeedsConversion));
    }

    #[test]
    fn test_parse_html_with_base_tag() {
        let html = r#"
            <base href="https://cdn.example.com/">
            <link rel="alternate" type="text/markdown" href="article.md">
        "#;
        let base = Url::parse("https://example.com/page").unwrap();
        let result = parse_html_for_markdown_link(html, &base).unwrap();

        match result {
            HtmlParseResult::MarkdownLink(url) => {
                assert_eq!(url.as_str(), "https://cdn.example.com/article.md");
            }
            _ => panic!("Expected MarkdownLink"),
        }
    }

    #[test]
    fn test_parse_json_url() {
        let json = r#"{"mdwn": "https://example.com/file.md"}"#;
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_json_for_markdown(json, &base).unwrap();

        match result {
            JsonParseResult::MarkdownUrl(url) => {
                assert_eq!(url.as_str(), "https://example.com/file.md");
            }
            _ => panic!("Expected MarkdownUrl"),
        }
    }

    #[test]
    fn test_parse_json_relative_url() {
        let json = r#"{"markdown": "/content/article.md"}"#;
        let base = Url::parse("https://example.com/api/").unwrap();
        let result = parse_json_for_markdown(json, &base).unwrap();

        match result {
            JsonParseResult::MarkdownUrl(url) => {
                assert_eq!(url.as_str(), "https://example.com/content/article.md");
            }
            _ => panic!("Expected MarkdownUrl"),
        }
    }

    #[test]
    fn test_parse_json_inline_content() {
        let json = r##"{"mdwn": "# Hello World"}"##;
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_json_for_markdown(json, &base).unwrap();

        match result {
            JsonParseResult::MarkdownContent(content) => {
                assert!(content.starts_with("# Hello World"));
            }
            _ => panic!("Expected MarkdownContent"),
        }
    }

    #[test]
    fn test_parse_json_no_field() {
        let json = r#"{"title": "Article", "content": "..."}"#;
        let base = Url::parse("https://example.com/").unwrap();
        let result = parse_json_for_markdown(json, &base).unwrap();

        assert!(matches!(result, JsonParseResult::NotFound));
    }

    #[test]
    fn test_categorize_content_type() {
        assert_eq!(categorize_content_type(Some("text/markdown")), ContentCategory::Markdown);
        assert_eq!(categorize_content_type(Some("text/x-markdown")), ContentCategory::Markdown);
        assert_eq!(categorize_content_type(Some("TEXT/MARKDOWN")), ContentCategory::Markdown);
        assert_eq!(categorize_content_type(Some("text/plain")), ContentCategory::PlainText);
        assert_eq!(categorize_content_type(Some("text/html")), ContentCategory::Html);
        assert_eq!(categorize_content_type(Some("application/json")), ContentCategory::Json);
        assert_eq!(
            categorize_content_type(Some("application/vnd.api+json")),
            ContentCategory::Json
        );
    }
}
