# MLH Parser

A Python tool for parsing raw email archives from the MLH Archiver into a structured Parquet columnar dataset.

## Overview

The MLH Parser processes email files produced by the MLH Archiver and converts them into an efficient, queryable Parquet dataset with Hive partitioning by mailing list name. It automatically detects and reads both `.eml` (RFC 822) and `.parquet` (columnar) input formats.

## Features

- **Parquet Output**: Columnar storage format optimized for analytics
- **Hive Partitioning**: Data organized by mailing list for efficient querying
- **Auto-detection of Input Format**: Reads `.eml` and `.parquet` files transparently from the same input directory
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

### Input Formats

The parser automatically detects and processes both input formats within each mailing list directory:

| Format | Extension | Description |
|--------|-----------|-------------|
| Raw email | `.eml` | Individual RFC 822 email files (one file per email) |
| Columnar | `.parquet` | Parquet files containing multiple emails in columnar form |

#### Parquet Input

When reading `.parquet` files, each file must contain the following columns:

| Column | Type | Description |
|--------|------|-------------|
| `email_id` | string | Unique identifier for each email |
| `content` | string / list\<string\> | Full raw email content |

Each row in the parquet file is yielded as an individual email for parsing, with the composite name `{email_id}:{parquet_filename}` used for provenance tracking.

Both `.eml` and `.parquet` files can coexist in the same input directory — the parser automatically dispatches to the correct reader based on file extension.

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
| `trailers` | list\<struct\<attribution: string, identification: string\>\> | Signature block attribution and identification |
| `code` | list\<string\> | Code snippets extracted from email |
| `raw_body` | string | Complete raw email body |

## Configuration

Configuration is done via environment variables. These can be set in your shell, in a `.env` file, or passed directly to the command.

### Runtime Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `INPUT_DIR` | `/parser/input` | Directory containing raw emails (container mode) |
| `OUTPUT_DIR` | `/parser/output` | Output directory for Parquet files (container mode) |
| `DEBUG` | `False` | Enable debug mode (sets `N_PROC=1` and enables verbose logging) |
| `N_PROC` | `cpu_count() / 2` | Number of parallel processes to use (ignored if `DEBUG=True`) |
| `REDO_FAILED_PARSES` | `False` | If `True`, re-parse only emails that previously failed |
| `LISTS_TO_PARSE` | `""` (all lists) | Comma-separated list of mailing lists to parse. Empty means parse all available lists. |

### Examples

```bash
# Run with debug mode (single process, verbose logging)
DEBUG=True uv run src/main.py

# Parse only specific mailing lists
LISTS_TO_PARSE="list1,list2,list3" uv run src/main.py

# Re-parse only failed emails from previous run
REDO_FAILED_PARSES=True uv run src/main.py

# Use 4 parallel processes
N_PROC=4 uv run src/main.py

# Native execution with custom directories
INPUT_DIR="../output" OUTPUT_DIR="../parser_output" uv run src/main.py

# Using Make with environment variables
make parse N_PROC=4
make parse LISTS_TO_PARSE="list1,list2,list3"
make parse REDO_FAILED_PARSES=true N_PROC=2
make debug-parser N_PROC=1 LISTS_TO_PARSE="dev.rcpassos.me.lists.gfs2"
```

### Notes

- **`DEBUG` mode**: When enabled, forces single-threaded execution (`N_PROC=1`) for easier debugging
- **`N_PROC`**: Defaults to half of available CPU cores for balanced performance
- **`LISTS_TO_PARSE`**: Useful for testing or incremental parsing of specific lists
- **`REDO_FAILED_PARSES`**: Reads from the `errors/` directory instead of the main input directory

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

### Test Structure

The test suite uses real email samples (`.eml` files) paired with expected output files (`.pytest` extension). This approach allows testing with actual mailing list emails while maintaining readable expected values.

#### Test File Organization

```
tests/
├── complete_cases/          # Full email parsing tests (trailers + code)
│   ├── 14.eml              # Raw email file
│   ├── 14.trailers.pytest  # Expected trailers (Python literal)
│   ├── 14.code.pytest      # Expected code/patches (Python literal)
│   ├── 14.body.pytest      # Expected email body
│   └── 14.headers.pytest   # Expected headers (raw format)
├── date_cases/              # Date parsing test cases
│   ├── org.kernel...6592.eml           # Raw email file
│   └── org.kernel...6592.date.pytest   # Expected parsed date
├── test_complete_parsers.py  # Test runner for complete cases
├── test_base_email_parsers.py  # Tests for body and header parsing
├── test_dates.py             # Test runner for date parsing
├── test_attributions.py      # Unit tests for attribution extraction
├── test_patches.py           # Unit tests for patch extraction
└── helpers.py                # Test utilities
```

#### File Naming Convention

Test files are grouped by a common prefix:

| File Pattern | Purpose |
|--------------|---------|
| `<prefix>.eml` | Raw RFC 822 email input |
| `<prefix>.trailers.pytest` | Expected trailers (Python list literal) |
| `<prefix>.code.pytest` | Expected code patches (Python list literal) |
| `<prefix>.body.pytest` | Expected email body |
| `<prefix>.headers.pytest` | Expected headers (raw format) |
| `<prefix>.date.pytest` | Expected parsed date (first line is the date) |
| `<prefix>.client-date.pytest` | Expected raw client dates (one per line) |

#### Adding New Test Cases

1. **Save the raw email**: Place your `.eml` file in the appropriate directory (`complete_cases/` or `date_cases/`)

2. **Create expected output files**: For each `.eml` file, create corresponding `.pytest` files with the expected parsed values as Python literals:

   ```python
   # Example: 14.trailers.pytest
   [
       {
           "attribution": "Signed-off-by",
           "identification": "Example Developer <example-dev@company.com>",
       },
   ]
   
   # Example: 14.code.pytest
   [
       """---
   drivers/file.c | 10 ++++++++++
   1 file changed, 10 insertions(+)
   ...
   """
   ]
   
   # Example: email.date.pytest
   Tue,  4 Nov 2025 22:14:47 +0000
   # Note: Additional lines are treated as comments
   ```

3. **Run tests**: The test runners automatically discover files by extension and match them by prefix.

#### Test Helpers

The `helpers.py` module provides utilities:

- `list_files_with_extension(directory, ext)`: List all files with given extension
- `map_to_file_extensions(email_file, extensions)`: Map `.eml` to its `.pytest` counterparts
- `resolve_test_file_path(directory, filename)`: Resolve absolute path to test file

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
