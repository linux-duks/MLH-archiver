# MLH Parser

A Python tool for parsing raw email archives from the MLH Archiver into a structured Parquet columnar dataset.

## Overview

The MLH Parser processes raw RFC 822 email files produced by the MLH Archiver and converts them into an efficient, queryable Parquet dataset with Hive partitioning by mailing list name.

## Features

- **Parquet Output**: Columnar storage format optimized for analytics
- **Hive Partitioning**: Data organized by mailing list for efficient querying
- **Email Field Extraction**: Parses headers, body, attachments, and metadata
- **Error Handling**: Failed parses are saved separately for review
- **Containerized**: Runs consistently across different environments

## Prerequisites

### Container Runtime (Required)

- Podman with Podman Compose, or
- Docker with Docker Compose

### Native Development (Optional)

- Python 3.14+
- [uv](https://docs.astral.sh/uv/) package manager
- Nox (for testing)

## Installation

### Using Devbox (Recommended)

```bash
devbox shell
```

This sets up Python 3.14, uv, and all required dependencies automatically.

### Manual Setup

```bash
# Install uv if not already installed
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install dependencies
uv sync --locked
```

## Usage

### Running the Parser

The parser expects raw email files from the archiver in the `../output` directory.

```bash
# Using Make
make run

# Using Devbox
devbox run parse

# Debug mode (native execution)
make debug-parser
# or
INPUT_DIR="../output" OUTPUT_DIR="../parser_output" uv run src/main.py
```

### Input/Output Directories

| Directory | Purpose |
|-----------|---------|
| `../output/` | Input: Raw email files from archiver |
| `../parser_output/parsed/` | Output: Parquet dataset |
| `../parser_output/<list>/errors/` | Failed parses |

## Output Format

The parser produces a Parquet dataset with Hive partitioning:

```
parser_output/parsed/
├── mailing_list=dev.rcpassos.me.lists.gfs2/
│   ├── part-0.parquet
│   └── part-1.parquet
├── mailing_list=dev.rcpassos.me.lists.iommu/
│   └── part-0.parquet
└── _common_metadata
```

### Schema

The Parquet dataset includes the following columns:

| Column | Type | Description |
|--------|------|-------------|
| `message-id` | string | Email Message-ID header |
| `from` | string | Sender email address |
| `to` | list\<string\> | Recipients (To field) |
| `cc` | list\<string\> | CC recipients |
| `subject` | string | Email subject line |
| `date` | datetime | Parsed email date (corrected) |
| `client-date` | list\<string\> | Raw date from email client (may be incorrect) |
| `in-reply-to` | string | In-Reply-To header |
| `references` | list\<string\> | References headers |
| `x-mailing-list` | string | Mailing list name |
| `trailers` | list | Signature block attribution and identification |
| `code` | list | Code snippets extracted from email |
| `raw_body` | string | Complete raw email body |

## Configuration

Configuration is done via environment variables when running in container mode:

| Variable | Default | Description |
|----------|---------|-------------|
| `INPUT_DIR` | `/parser/input` | Directory containing raw emails |
| `OUTPUT_DIR` | `/parser/output` | Output directory for Parquet files |

## Development

### Running Tests

```bash
# Using Make
make test

# Using Devbox
devbox run test-parser

# Native with nox
nox

# Native with pytest
uv run pytest
```

### Debug Mode

Run the parser directly without containers for debugging:

```bash
INPUT_DIR="../output" OUTPUT_DIR="../parser_output" uv run src/main.py
```

### Project Structure

```
mlh_parser/
├── src/
│   ├── mlh_parser/
│   │   ├── __init__.py      # Module exports
│   │   ├── parser.py        # Main parsing logic
│   │   ├── parser_algorithm.py  # Core algorithm
│   │   ├── email_reader.py  # Email file reading
│   │   ├── date_parser.py   # Date parsing utilities
│   │   └── constants.py     # Configuration constants
│   ├── main.py              # Entry point
│   └── sanity_check.py      # Validation utilities
├── tests/                   # Test suite
├── Containerfile            # Docker/Podman image
├── compose.yaml             # Container orchestration
├── pyproject.toml           # Python project configuration
├── uv.lock                  # Locked dependencies
├── noxfile.py               # Test automation
└── Makefile                 # Build automation
```

## Dependencies

- `polars` (~1.39) - Fast DataFrame library for data processing
- `python-dateutil` (>=2.9.0) - Date parsing utilities
- `tqdm` (~4.67) - Progress bars

### Development Dependencies

- `pytest` (>=9.0) - Testing framework
- `nox` (>=2026.2) - Test automation
- `freezegun` (>=1.5) - Time mocking for tests

## Container Build

The parser runs in a container using the `ghcr.io/astral-sh/uv:python3.14-trixie-slim` base image.

```bash
# Rebuild container image
make rebuild

# Or with devbox
devbox run rebuild
```

## Error Handling

Emails that fail to parse are saved to:

```
parser_output/<mailing_list>/errors/
```

This allows you to:

- Identify problematic emails
- Debug parsing issues
- Re-process fixed emails separately

## Integration with Other Components

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  MLH Archiver   │ ──► │   MLH Parser    │ ──► │   Anonymizer    │
│  (raw emails)   │     │  (Parquet DS)   │     │ (anonymized DS) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

1. Run archiver to collect raw emails: `make run`
2. Run parser to create dataset: `make parse`
3. Run anonymizer for privacy: `make anonymize`

## Example Usage with Polars

```python
import polars as pl

# Read the parsed dataset
df = pl.scan_parquet("../parser_output/parsed/**/*.parquet")

# Query emails by subject
result = (
    df
    .filter(pl.col("subject").str.contains("example"))
    .select(["date", "from", "subject"])
    .collect()
)
```

## Troubleshooting

### "Input directory is missing or empty"

Run the archiver first to generate raw email files:

```bash
make run
```

### Container Permission Issues

The compose file uses `user: "${UID}:${GID}"` to match your user ID. Ensure your user has read/write access to the input/output directories.

### Parsing Errors

Check the `errors/` directory for failed emails. Common issues:

- Malformed email headers
- Unsupported character encodings
- Corrupted email files

## License

See the root [LICENSE](../LICENSE) file.
