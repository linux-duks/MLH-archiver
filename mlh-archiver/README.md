# MLH Archiver

A multi-threaded Rust application for archiving mailing list emails from NNTP (Network News Transfer Protocol) servers to local storage.

## Overview

The MLH Archiver connects to NNTP servers and downloads emails from specified mailing lists, storing them as raw email files. It's designed to be respectful to NNTP servers by not fetching emails too aggressively, avoiding detection as a malicious scraping bot.

## Architecture

Each worker thread handles one mailing list at a time, fetching one email at a time. This design ensures:

- Respectful bandwidth usage
- Ability to keep local files up-to-date with new articles
- Parallel processing across multiple mailing lists

See the [architecture diagram](../docs/fluxogram.svg) for a visual representation of the workflow.

## Features

- **Multi-threaded**: Process multiple mailing lists concurrently
- **Configurable**: Support for JSON, YAML, and TOML configuration files
- **Interactive TUI**: Select mailing lists from an interactive terminal interface
- **Flexible article selection**: Read specific article ranges or all articles
- **Continuous or one-shot mode**: Loop to keep archives updated or run once

## Prerequisites

### Native Build

- Rust toolchain (cargo, rustc)
- `libiconv` (for character encoding support)

### Container Build (Alternative)

- Podman or Docker
- No Rust installation required

## Building

### Using Make

```bash
# Build the archiver
make build

# Build and run
make run
```

### Using Devbox

```bash
devbox run build
devbox run run
```

### Manual Build

```bash
# Native build
cargo build --release

# Container build with Podman
podman run --rm -it -u $(id -u):$(id -g) \
  --network=host \
  -v ./:/usr/src/app:z \
  -w /usr/src/app \
  docker.io/rust:1.94-slim \
  cargo build --release
```

The compiled binary will be at `target/release/mlh-archiver`.

## Usage

### Command Line Arguments

```bash
Usage: mlh-archiver [OPTIONS]

Options:
  -c, --config-file <CONFIG_FILE>      Path to config file [default: archiver_config*]
  -h, --help                           Print help
```

**Note:** All configuration is done via the config file.

### Environment Variables

- `RUST_LOG` - Log level (e.g., `debug`, `info`, `warn`, `error`)

### Examples

```bash
# Using a config file
cargo run -- -c archiver_config.yaml

# With debug logging
RUST_LOG=debug cargo run -- -c archiver_config.yaml
```

## Configuration

The archiver looks for configuration files matching `archiver_config*.{json,yaml,toml}` in the current directory by default.

Configuration is **nested**: global settings at the top level, NNTP-specific settings under the `nntp:` block.

### Example YAML Configuration

```yaml
# archiver_config.yaml
nthreads: 2
output_dir: "./output"
loop_groups: true

nntp:
  hostname: "nntp.example.com"
  port: 119
  group_lists:
    - dev.rcpassos.me.lists.gfs2
    - dev.rcpassos.me.lists.iommu
```

### Configuration Options

#### Global Options

| Option | Type | Description |
|--------|------|-------------|
| `nthreads` | integer | Number of parallel worker threads (default: 1) |
| `output_dir` | string | Directory to store archived emails (default: "./output") |
| `loop_groups` | boolean | Continuously check for new articles (default: true) |

#### NNTP Options (under `nntp:` block)

| Option | Type | Description |
|--------|------|-------------|
| `hostname` | string | **Required.** NNTP server hostname or IP |
| `port` | integer | NNTP server port (default: 119) |
| `group_lists` | list | Mailing list names to archive (e.g., `["ALL"]` or specific lists) |
| `article_range` | string | Optional. Read specific range of articles (e.g., `"1-100"` or `"1,5,10-20"`) |

## Output Format

Emails are stored as raw RFC 822 email files in the output directory, organized by mailing list:

```
output/
├── dev.rcpassos.me.lists.gfs2/
│   ├── 000001.eml
│   ├── 000002.eml
│   └── ...
└── dev.rcpassos.me.lists.iommu/
    ├── 000001.eml
    └── ...
```

## Testing

```bash
# Run tests
make test

# Or with devbox
devbox run test-archiver

# Or directly
cargo test
```

## Project Structure

```
mlh-archiver/
├── src/
│   ├── main.rs          # Application entry point
│   ├── config.rs        # Configuration loading
│   ├── scheduler.rs     # Worker thread scheduling
│   ├── worker.rs        # NNTP fetching logic
│   ├── file_utils.rs    # File I/O utilities
│   ├── range_inputs.rs  # Article range parsing
│   └── errors.rs        # Error types
├── rust-nntp/           # Forked NNTP library
├── tests/               # Integration tests
├── Cargo.toml           # Rust dependencies
└── Makefile             # Build automation
```

## Dependencies

- `clap` - Command line argument parsing
- `config` - Configuration file loading (JSON, YAML, TOML)
- `crossbeam-channel` - Thread communication
- `env_logger` - Logging with environment variable support
- `inquire` - Interactive TUI prompts
- `nntp` - NNTP protocol implementation (forked, local)
- `serde` / `serde_yaml` - Serialization
- `chrono` - Date/time handling
- `testcontainers` - Integration testing with containers

## Troubleshooting

### Connection Issues

- Verify NNTP server hostname and port in your config file
- Check firewall rules for NNTP traffic (typically port 119 or 563 for SSL)
- Some NNTP servers require authentication (not currently supported)

### Configuration Issues

- Ensure `nntp.hostname` is set in your config file
- The `nntp:` block is required
- Check that YAML syntax is valid

### Build Issues

- Ensure `libiconv` is installed for character encoding support
- For container builds, verify Podman/Docker is running

### Logging

Enable debug logging for troubleshooting:

```bash
RUST_LOG=debug cargo run -- -c archiver_config.yaml
```

## License

See the root [LICENSE](../LICENSE) file.
