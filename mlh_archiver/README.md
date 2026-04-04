# MLH Archiver

A multi-threaded Rust application for archiving mailing list emails from NNTP (Network News Transfer Protocol) servers to local storage.

## Overview

The MLH Archiver connects to NNTP servers and downloads emails from specified mailing lists, storing them as raw email files. It's designed to be respectful to NNTP servers by not fetching emails too aggressively, avoiding detection as a malicious scraping bot.

## Architecture

The MLH Archiver uses a producer-consumer pattern with multiple worker threads:

### Worker Model

- **Workers** are created in `lib.rs::start()` and owned by `WorkerManager`
- Each worker is **moved to its own thread** before execution
- Workers receive tasks via **crossbeam channels** (one channel per worker group)
- **Shutdown** is coordinated via `Arc<AtomicBool>` flag passed from `main.rs`

### Thread Communication

```
Producer Thread ──► Sender<Group> ──► Receiver<Group> (cloned to each worker)
                                           ├─► Worker 1 (thread 1)
                                           ├─► Worker 2 (thread 2)
                                           └─► Worker N (thread N)
```

When a task (mailing list name) is sent to the channel, only **one** worker receives it,
enabling natural load balancing.

### Shutdown Mechanism

1. Ctrl+C signal sets shared `AtomicBool` flag in `main.rs`
2. Flag is cloned to each worker at creation time
3. Workers check flag:
   - At start of each task iteration
   - During reconnection waits (60s)
   - During error recovery waits (10s)
   - During email fetching (per article)

### Design Principles

- **Respectful bandwidth**: Not designed to fetch as fast as possible
- **Continuous operation**: Can keep local files up-to-date with new articles
- **Graceful shutdown**: Clean exit on Ctrl+C with progress preservation

See the [architecture diagram](../docs/fluxogram.svg) for a visual representation.

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

The compiled binary will be at `target/release/mlh_archiver`.

## Usage

### Command Line Arguments

```bash
Usage: mlh_archiver [OPTIONS]

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
| `group_lists` | list | Mailing list names to archive (e.g., `["*"]` for all, or specific lists/globs) |
| `article_range` | string | Optional. Read specific range of articles (e.g., `"1-100"` or `"1,5,10-20"`) |
| `username` | string | Optional. NNTP server username for authentication |
| `password` | string | Optional. NNTP server password for authentication |

## Article Range Selection

The `article_range` configuration option allows fetching specific articles instead of all new emails:

```yaml
nntp:
  hostname: "nntp.example.com"
  article_range: "1,5,10-15"  # Fetch articles 1, 5, and 10-15
```

**Supported formats:**
- Single numbers: `"100"`
- Ranges: `"1-50"`
- Comma-separated: `"1,5,10"`
- Mixed: `"1,3-5,10-15"`

**Memory efficiency:** Range parsing is lazy - the range string is stored and parsed per mailing list, avoiding memory issues with large ranges.

**Use cases:**
- Retry failed articles: `article_range: "42,108,256"`
- Fetch specific date ranges (if you know article numbers)
- Test runs with small samples: `article_range: "1-10"`

## Authentication

If your NNTP server requires authentication, provide credentials in the config:

```yaml
nntp:
  hostname: "nntp.example.com"
  port: 563
  username: "myuser"
  password: "mypass"
  group_lists: ["*"]
```

Both `username` and `password` are optional. If omitted, the archiver connects without authentication.

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
# Run all tests
make test

# Or with devbox
devbox run test-archiver

# Or directly
cargo test
```

### Test Coverage

**Unit Tests** (`cargo test --lib`):
- Range parsing (`range_inputs.rs`)
- Configuration loading and validation
- Error types

**Integration Tests** (`cargo test --test test_nntp`):
- Full list download from mock NNTP server
- Single article by range (`"5"`)
- Article range (`"1-3"`)
- Multiple articles (`"1,5,10"`)
- Mixed ranges (`"1,3-5,10"`)

Integration tests use testcontainers to spin up a mock NNTP server. Requires Docker/Podman.

## Documentation

### Rust API Documentation

Generate and open the Rust API documentation in your browser:

```bash
# Using make
make doc

# Using devbox
devbox run doc

# Or directly
cargo doc --document-private-items --open
```

This generates comprehensive documentation including:
- All public and private items
- Function signatures with parameters and return values
- Struct and enum field descriptions
- Usage examples where provided
- Intra-doc links between modules

Documentation is output to `target/doc/mlh_archiver/` and automatically opened in your default browser.

## Project Structure

```
mlh_archiver/
├── src/
│   ├── main.rs              # Application entry point, Ctrl+C handler
│   ├── lib.rs               # Core start() function, worker initialization
│   ├── config.rs            # Configuration loading and RunMode handling
│   ├── scheduler.rs         # Thread orchestration, producer/consumer pattern
│   ├── worker.rs            # Worker trait, WorkerManager ownership
│   ├── errors.rs            # Error types (Error, ConfigError)
│   ├── file_utils.rs        # File I/O, YAML serialization
│   ├── range_inputs.rs      # Article range parsing (lazy iterator)
│   └── nntp_source/         # NNTP-specific implementation
│       ├── mod.rs           # Module exports, NNTP connection helper
│       ├── nntp_config.rs   # NNTP configuration struct
│       ├── nntp_lister.rs   # List retrieval from NNTP server
│       ├── nntp_utils.rs    # shared utils handling the NNTP lib
│       └── nntp_worker.rs   # NNTPWorker implementation
├── rust-nntp/               # Forked NNTP library
├── tests/
│   ├── test_config.rs       # Configuration tests
│   ├── test_nntp.rs         # Integration tests with testcontainers
│   └── test_shutdown.rs     # Shutdown flag tests
├── Cargo.toml               # Rust dependencies
└── Makefile                 # Build automation
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

## Development: Implementing a New Source

To add a new email source (e.g., ListArchiveX, IMAP, local mbox), follow these steps:

### 1. Create Source Module

Create `src/list_archive_x_source/` (or your source name):

```
src/
└── list_archive_x_source/
    ├── mod.rs
    ├── list_archive_x_config.rs
    └── list_archive_x_worker.rs
```

### 2. Implement Configuration

**`src/list_archive_x_source/list_archive_x_config.rs`:**

```rust
use crate::errors::ConfigError;

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct ListArchiveXConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub group_lists: Option<Vec<String>>,
    pub article_range: Option<String>,
}

impl ListArchiveXConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.base_url.is_empty() {
            return Err(ConfigError::MissingHostname);
        }
        Ok(())
    }
}
```

### 3. Implement Worker

**`src/list_archive_x_source/list_archive_x_worker.rs`:**

```rust
use crate::worker::Worker;
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

pub struct ListArchiveXWorker {
    id: u8,
    config: ListArchiveXConfig,
    shutdown_flag: Arc<AtomicBool>,
    // ... other fields (e.g., HTTP client)
}

impl ListArchiveXWorker {
    pub fn new(
        id: u8,
        config: ListArchiveXConfig,
        base_output_path: String,
        shutdown_flag: Arc<AtomicBool>,
    ) -> Self {
        ListArchiveXWorker {
            id,
            config,
            shutdown_flag,
            // ...
        }
    }
}

impl Worker for ListArchiveXWorker {
    fn consumme_list(
        self: Box<Self>,
        receiver: crossbeam_channel::Receiver<String>,
    ) -> crate::Result<()> {
        loop {
            // Check shutdown flag at start of each iteration
            if self.shutdown_flag.load(Ordering::Relaxed) {
                log::info!("W{}: Shutdown requested, exiting...", self.id);
                return Ok(());
            }

            // Receive task from channel
            let list_name = match receiver.recv() {
                Ok(name) => name,
                Err(_) => return Ok(()), // Channel closed
            };

            // Fetch emails for list_name...
        }
    }

    fn read_email_by_index(
        &self,
        list_name: String,
        email_index: usize,
    ) -> crate::Result<()> {
        // Implement single email retrieval
        // ...
        Ok(())
    }
}
```

**Key requirements:**
- Store `shutdown_flag: Arc<AtomicBool>` for graceful shutdown
- Check shutdown flag at:
  - Start of each task iteration
  - During long waits or retries
  - During email fetching loops
- Use `RefCell` or `Mutex` for mutable connection state

### 4. Update Configuration

**`src/config.rs`:**

Add new variant to enums:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    NNTP,
    ListArchiveX,  // Add this
    LocalMbox,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunModeConfig {
    NNTP(nntp_config::NntpConfig),
    ListArchiveX(list_archive_x_config::ListArchiveXConfig),  // Add this
    LocalMbox,
}
```

Add to `AppConfig` struct:

```rust
pub struct AppConfig {
    // ... existing fields
    pub list_archive_x: Option<list_archive_x_config::ListArchiveXConfig>,
}
```

Update `get_run_mode_config()`:

```rust
pub fn get_run_mode_config(&self, run_mode: RunMode) -> Option<RunModeConfig> {
    match run_mode {
        RunMode::NNTP => Some(RunModeConfig::NNTP(self.nntp.clone()?)),
        RunMode::ListArchiveX => Some(RunModeConfig::ListArchiveX(self.list_archive_x.clone()?)),
        RunMode::LocalMbox => Some(RunModeConfig::LocalMbox),
    }
}
```

Update `get_run_modes()`:

```rust
pub fn get_run_modes(&self) -> Vec<RunMode> {
    let mut run_modes = vec![];
    if self.nntp.is_some() {
        run_modes.push(RunMode::NNTP);
    }
    if self.list_archive_x.is_some() {
        run_modes.push(RunMode::ListArchiveX);
    }
    run_modes
}
```

### 5. Register Worker

**`src/worker.rs`:**

```rust
use crate::list_archive_x_source::list_archive_x_worker::ListArchiveXWorker;

impl WorkerManager {
    pub fn create_workers(
        &mut self,
        run_mode: RunMode,
        tasks: Vec<String>,
        app_config: &AppConfig,
        shutdown_flag: Arc<AtomicBool>,
    ) {
        match run_mode {
            RunMode::NNTP => { /* existing */ }
            RunMode::ListArchiveX => {
                if let Some(RunModeConfig::ListArchiveX(config)) =
                    app_config.get_run_mode_config(run_mode)
                {
                    let num_workers = app_config.nthreads.max(1) as usize;
                    for id in 0..num_workers {
                        let worker = ListArchiveXWorker::new(
                            id as u8,
                            config.clone(),
                            app_config.output_dir.clone(),
                            shutdown_flag.clone(),
                        );
                        workers.push(Box::new(worker));
                    }
                }
            }
            RunMode::LocalMbox => { /* existing */ }
        }
    }
}
```

### 6. Update Module Exports

**`src/lib.rs`:**

```rust
pub mod list_archive_x_source;
```

### 7. Update Configuration File Format

Document new config structure:

```yaml
nthreads: 2
output_dir: "./output"
loop_groups: true

nntp:
  hostname: "nntp.example.com"
  port: 119
  group_lists: ["list1"]

list_archive_x:
  base_url: "https://archive.example.com/api"
  api_key: "your-api-key"
  group_lists: ["list1", "list2"]
```

### 8. Add Tests

Create `tests/test_list_archive_x.rs` following the pattern in `tests/test_nntp.rs`.

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
