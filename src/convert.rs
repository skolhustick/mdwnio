use crate::error::{MdwnError, Result};
use readability::extractor;
use url::Url;

/// Notice prepended to converted markdown content
const CONVERSION_NOTICE: &str =
    "<!-- mdwn.io: Converted from HTML. Original may have richer formatting. -->\n\n";

/// Convert HTML to markdown using readability extraction
pub fn html_to_markdown(html: &str, base_url: &Url) -> Result<String> {
    // Use readability to extract main content
    let product = extractor::extract(&mut html.as_bytes(), base_url)
        .map_err(|e| MdwnError::ParseError(format!("Readability extraction failed: {}", e)))?;

    // Convert the extracted HTML to markdown
    let markdown = htmd::convert(&product.content)
        .map_err(|e| MdwnError::ParseError(format!("HTML to Markdown conversion failed: {}", e)))?;

    // Clean up the markdown
    let markdown = clean_markdown(&markdown);

    // Prepend title if available
    let markdown = if !product.title.is_empty() {
        format!("# {}\n\n{}", product.title, markdown)
    } else {
        markdown
    };

    // Add conversion notice
    let markdown = format!("{}{}", CONVERSION_NOTICE, markdown);

    Ok(markdown)
}

/// Clean up converted markdown
fn clean_markdown(md: &str) -> String {
    let mut result = String::with_capacity(md.len());
    let mut prev_blank = false;
    let lines = md.lines().peekable();

    for line in lines {
        let trimmed = line.trim();

        // Skip excessive blank lines (keep max 2 consecutive)
        if trimmed.is_empty() {
            if prev_blank {
                continue;
            }
            prev_blank = true;
        } else {
            prev_blank = false;
        }

        result.push_str(line);
        result.push('\n');
    }

    // Trim trailing whitespace but keep one final newline
    let result = result.trim_end();
    format!("{}\n", result)
}

/// Check if the HTML content appears to be meaningful (not just a JS shell)
pub fn is_meaningful_html(html: &str) -> bool {
    // Simple heuristic: check if there's actual text content
    // JS-rendered pages often have very little text in the initial HTML
    let text_content: String = html
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();

    // If there's very little text, it's probably a JS shell
    let word_count = text_content.split_whitespace().count();
    word_count > 20 // Threshold for meaningful content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_html_to_markdown() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head><title>Test Article</title></head>
            <body>
                <article>
                    <h1>Test Article</h1>
                    <p>This is a paragraph with <strong>bold</strong> and <em>italic</em> text.</p>
                    <ul>
                        <li>Item 1</li>
                        <li>Item 2</li>
                    </ul>
                </article>
            </body>
            </html>
        "#;
        let base = Url::parse("https://example.com/").unwrap();
        let result = html_to_markdown(html, &base).unwrap();

        assert!(result.contains("<!-- mdwn.io:"));
        assert!(result.contains("**bold**") || result.contains("bold"));
    }

    #[test]
    fn test_clean_markdown() {
        let messy = "# Title\n\n\n\n\nParagraph\n\n\n\nAnother";
        let clean = clean_markdown(messy);

        // Should not have more than 2 consecutive newlines
        assert!(!clean.contains("\n\n\n\n"));
    }

    #[test]
    fn test_is_meaningful_html() {
        // Meaningful HTML
        let good_html = r#"
            <html>
            <body>
                <article>
                    <p>This is a real article with actual content that people might want to read.
                    It contains multiple sentences and paragraphs of meaningful text.</p>
                </article>
            </body>
            </html>
        "#;
        assert!(is_meaningful_html(good_html));

        // JS shell (minimal content)
        let js_shell = r#"
            <html>
            <head><script src="app.js"></script></head>
            <body><div id="root"></div></body>
            </html>
        "#;
        assert!(!is_meaningful_html(js_shell));
    }
}
