# Include shared container runtime detection
include ../containers.mk

# Define the name of your binary
BINARY_NAME = mlh_parser

# CURDIR is the directory where make is running (after any -C changes)
# Repository root is parent of current directory
REPO_ROOT := $(abspath $(CURDIR)/..)
# When called with "make -C mlh_parser", CURDIR = mlh_parser/
# TARGET_PATH is the absolute path to the binary
TARGET_PATH := $(REPO_ROOT)/target/release/$(BINARY_NAME)


# Define the Docker image to use for building
RUST_DOCKER_IMAGE = docker.io/rust:1.94-slim


# ------------------------------------------------------------------------------
# BUILD LOGIC
# ------------------------------------------------------------------------------
.PHONY: build
build:
	@if command -v cargo >/dev/null 2>&1; then \
		echo "==> Found Rust toolchain, building natively..."; \
		cargo build --release; \
	else \
		echo "==> Rust toolchain not found, building with Docker (Image: $(RUST_DOCKER_IMAGE))..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-v ../:/usr/src/app:z \
			-w /usr/src/app \
			$(RUST_DOCKER_IMAGE) \
			cargo build --release; \
	fi

.PHONY: rebuild
rebuild: clean build

.PHONY: run
run:
	@if [ ! -f $(TARGET_PATH) ]; then \
		echo "==> Binary not found. Building first..."; \
		$(MAKE) build; \
	fi
	@echo "==> Running application from repository root..."
	@cd $(REPO_ROOT) && $(TARGET_PATH)


.PHONY: debug
debug:
	@if command -v cargo >/dev/null 2>&1; then \
		echo "==> Found Rust toolchain, building natively..."; \
		cd $(REPO_ROOT) && RUST_LOG=debug cargo run --bin $(BINARY_NAME); \
	else \
		echo "==> Rust toolchain not found, building with Docker (Image: $(RUST_DOCKER_IMAGE))..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			-e RUST_LOG=debug \
			--network=host \
			-v $(REPO_ROOT):/usr/src/app:z \
			-w /usr/src/app \
			$(RUST_DOCKER_IMAGE) \
			cargo run --bin $(BINARY_NAME); \
	fi


.PHONY: test
test:
	@echo "==> Running parser tests..."
	@cargo test

.PHONY: doc
doc:
	@echo "==> Generating Rust API documentation..."
	@cargo doc --document-private-items --open

.PHONY: clean
clean:
	@echo "==> Cleaning up parser build artifacts..."
	@cargo clean
	@rm -f ./$(BINARY_NAME).out


