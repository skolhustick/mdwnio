# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability in mdwn.io, please report it responsibly.

### How to Report

1. **DO NOT** open a public GitHub issue for security vulnerabilities
2. Email security concerns to: security@mdwn.io (or open a private security advisory on GitHub)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes (optional)

### What to Expect

- We will acknowledge receipt within 48 hours
- We will provide an initial assessment within 7 days
- We will work with you to understand and resolve the issue
- We will credit you (if desired) when the fix is released

## Security Measures

mdwn.io implements several security measures:

### SSRF Protection

- Only `http://` and `https://` schemes allowed
- Private IP ranges blocked (10.x, 172.16-31.x, 192.168.x, 127.x, 169.254.x)
- AWS metadata endpoint blocked (169.254.169.254)
- DNS resolution checked before connection
- Redirect targets re-validated

### Input Validation

- URL validation and sanitization
- Content length limits (default 10MB)
- Request timeouts (default 10s)
- Maximum redirect depth (default 5)

### No Sensitive Data

- No database or persistent storage
- No user authentication
- No cookies or sessions
- All configuration via environment variables

## Dependency Security

We use Dependabot to automatically check for security updates in our dependencies.

## Responsible Disclosure

We follow responsible disclosure practices:

1. Vulnerabilities are fixed privately before public disclosure
2. Credit is given to reporters (with permission)
3. CVEs are requested for significant vulnerabilities
4. Users are notified through GitHub releases
