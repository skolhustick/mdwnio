# mdwn.io

A lightweight Rust proxy that serves the markdown web. Point it at any URL, get markdown back.

## What it does

`mdwn.io/https://example.com/article` → returns markdown

## Content detection

Based on response content-type:

- `text/markdown` → pass through as-is
- `text/plain` → pass through as-is  
- `text/html` → parse for `<link rel="alternate" type="text/markdown" href="...">`, fetch that URL. If not found, extract content via readability and convert to markdown.
- `application/json` → look for top-level `mdwn` or `markdown` field containing URL or inline content

Response includes `X-Mdwn-Source` header: `native` (site provided markdown) or `converted` (extracted via readability).

## Stack

- **Rust + Axum** - web server
- **reqwest** - upstream fetching
- **scraper** - HTML parsing
- **moka** - in-memory TTL cache (1hr default)

No database. No external dependencies. Single binary.

## Project structure
```
mdwnio/
├── src/
│   ├── main.rs        # axum routes, config
│   ├── fetch.rs       # http client, SSRF protection
│   ├── parse.rs       # extract md url from html/json
│   ├── convert.rs     # readability + html-to-markdown
│   ├── cache.rs       # moka cache wrapper
│   └── error.rs       # error types
├── k8s/               # kubernetes manifests
├── Cargo.toml
├── Dockerfile
├── docker-compose.yml
├── readme.md          # this file, also served at /
└── LICENSE            # MIT
```

## Routes

- `GET /` → returns this README.md
- `GET /{url}` → proxies and returns markdown for that URL

## Config (env vars)

- `PORT` - default 3000
- `CACHE_TTL` - seconds, default 3600
- `REQUEST_TIMEOUT` - seconds, default 10
- `MAX_CONTENT_LENGTH` - bytes, default 10MB
- `MAX_REDIRECTS` - default 5
- `USER_AGENT` - default `mdwn.io/1.0 (+https://mdwn.io)`

## Security

- URL sanitization (no SSRF to localhost/internal IPs)
- Request timeouts
- Content length limits
- Memory-safe Rust

## Docker
```bash
docker run -p 3000:3000 ghcr.io/skolhustick/mdwnio:latest
```

## For publishers

Add this to your HTML to provide native markdown:
```html
<link rel="alternate" type="text/markdown" href="/path/to/article.md">
```

For JSON APIs, include a top-level field:
```json
{
  "mdwn": "https://example.com/content.md"
}
```

To whitelist mdwn.io requests, allow User-Agent: `mdwn.io/1.0 (+https://mdwn.io)`

## Self-host

Anyone can run their own instance. No central dependency on mdwn.io.
