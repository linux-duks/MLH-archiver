# if Podman is available, prefer using it over docker
ifeq ($(shell command -v podman 2> /dev/null),)
    CONTAINER=docker
else
    CONTAINER=podman
endif

# Define the name of your binary
BINARY_NAME = mlh-archiver

# Define the source path of the compiled binary
TARGET_PATH = ./target/release/$(BINARY_NAME)

# Define the Docker image to use for building
RUST_DOCKER_IMAGE = docker.io/rust:1.91-slim
PYTHON_DOCKER_IMAGE = ghcr.io/astral-sh/uv:python3.14-trixie-slim

# ==============================================================================

# By default, 'make' will run the 'all' target
.PHONY: all
all: build run

# ------------------------------------------------------------------------------
# BUILD LOGIC
# ------------------------------------------------------------------------------
# This target checks if 'cargo' is available in the system PATH.
# - If YES: It builds natively using 'cargo build'.
# - If NO:  It builds using the specified Docker image.
#
# After a successful build (either way), it copies the binary
# from the 'mlh-archiver/target/release' directory to the project root.
#
# ------------------------------------------------------------------------------
.PHONY: build
build:
	@if command -v cargo >/dev/null 2>&1; then \
		echo "==> Found Rust toolchain, building natively..."; \
		cd mlh-archiver && \
		cargo build --release; \
	else \
		echo "==> Rust toolchain not found, building with Docker (Image: $(RUST_DOCKER_IMAGE))..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-v ./:/usr/src/app:z \
			-w /usr/src/app \
			$(RUST_DOCKER_IMAGE) \
			cargo build --release; \
	fi
	@echo "==> Copying binary '$(BINARY_NAME).out' from target..."
	cp $(TARGET_PATH) ./$(BINARY_NAME).out

# ------------------------------------------------------------------------------
# UTILITY TARGETS
# ------------------------------------------------------------------------------

# Clean up build artifacts
.PHONY: clean
clean:
	@echo "==> Cleaning up build artifacts..."
	@cargo clean
	@rm -f ./$(BINARY_NAME).out


# ------------------------------------------------------------------------------
# APPLICATION TARGETS
# ------------------------------------------------------------------------------


.PHONY: run
run:
	@if [ ! -f ./$(BINARY_NAME).out ]; then \
		echo "==> Binary './$(BINARY_NAME).out' not found. Building first..."; \
		$(MAKE) build; \
	fi
	@echo "==> Running application..."
	@./$(BINARY_NAME).out

# Target to run the parser
# Checks if 'output' dir exists and is not empty, then runs docker-compose.
.PHONY: parse
parse:
	@if [ ! -d "output" ] || [ -n "$(ls "output")" ]; then \
		echo "==> Error: 'output' directory is missing or empty."; \
		echo "==> Please run the archiver first to generate files."; \
		exit 1; \
	else \
		echo "==> Found files in 'output'. Changing to 'parser' directory..."; \
		cd mlh_parser && $(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi

.PHONY: anonymize 
anonymize:
	@if [ ! -d "parser_output/parsed" ] || [ -n "$(ls "parser_output/parsed")" ]; then \
		echo "==> Error: 'parser_output/parsed' directory is missing or empty."; \
		echo "==> Please run the parser first to generate the dataset."; \
		exit 1; \
	else \
		echo "==> Found files in 'parser_output/parsed'. Changing to 'anonymizer' directory..."; \
		cd anonymizer && $(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi

.PHONY: analysis 
analysis:
	@if [ ! -d "parser_output/parsed" ] || [ -n "$(ls "parser_output/parsed")" ]; then \
		echo "==> Error: 'parser_output/parsed' directory is missing or empty."; \
		echo "==> Please run the parser first to analyse the dataset."; \
		exit 1; \
	else \
		echo "==> Found files in 'parser_output/parsed'. Changing to 'analysis' directory..."; \
		cd analysis && $(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi

rebuild-anonymizer:
	cd anonymizer && $(CONTAINER)-compose build
rebuild-parser:
	cd mlh_parser && $(CONTAINER)-compose build
rebuild-analysis:
	cd analysis && $(CONTAINER)-compose build

rebuild: rebuild-parser rebuild-analysis rebuild-parser

debug-parser:
	cd mlh_parser && INPUT_DIR="../output" OUTPUT_DIR="../parser_output" uv run src/main.py


debug-anonimyzer:
	cd anonymizer && INPUT_DIR="../parser_output/parsed" OUTPUT_DIR="../anonymizer_output" uv run src/main.py

# TESTS
#

test-parser:
	@if command -v noxx >/dev/null 2>&1; then \
		echo "==> Found Python Testing toolchain, running natively..."; \
		cd mlh_parser  && nox; \
	else \
		echo "==> Python Testing toolchain not found, running with Docker (Image: $(PYTHON_DOCKER_IMAGE))..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-v ./mlh_parser/:/usr/src/app:z \
			-w /usr/src/app \
			$(PYTHON_DOCKER_IMAGE) \
			bash -c "uv tool install nox && uv sync --locked && nox"; \
	fi
