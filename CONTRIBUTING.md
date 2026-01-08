# Contributing to mdwn.io

Thank you for your interest in contributing to mdwn.io!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/mdwn.git`
3. Create a branch: `git checkout -b feature/your-feature`
4. Make your changes
5. Run tests: `cargo test`
6. Run lints: `cargo clippy`
7. Format code: `cargo fmt`
8. Commit your changes
9. Push to your fork
10. Open a Pull Request

## Development Setup

### Prerequisites

- Rust 1.83 or later
- Docker (optional, for containerized testing)

### Building

```bash
cargo build
```

### Running locally

```bash
cargo run
```

The server will start on `http://localhost:3000`.

### Running tests

```bash
cargo test
```

## Code Style

- Follow Rust conventions
- Run `cargo fmt` before committing
- Run `cargo clippy` and fix any warnings
- Write tests for new functionality
- Keep commits focused and atomic

## Pull Request Guidelines

- Keep PRs focused on a single change
- Write clear commit messages
- Update documentation if needed
- Add tests for new features
- Ensure CI passes

## Reporting Issues

- Check existing issues first
- Include steps to reproduce
- Include relevant logs/errors
- Specify your environment (OS, Rust version)

## Security Issues

Please report security vulnerabilities privately. See [SECURITY.md](SECURITY.md) for details.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.
