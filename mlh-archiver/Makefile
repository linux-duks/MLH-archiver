# Include shared container runtime detection
include ../containers.mk

# Define the name of your binary
BINARY_NAME = mlh-archiver

# Define the source path of the compiled binary
TARGET_PATH = ./target/release/$(BINARY_NAME)


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
			-v ./:/usr/src/app:z \
			-w /usr/src/app \
			$(RUST_DOCKER_IMAGE) \
			cargo build --release; \
	fi

.PHONY: run
run:
	@if [ ! -f ../$(TARGET_PATH) ]; then \
		echo "==> Binary './$(BINARY_NAME).out' not found. Building first..."; \
		$(MAKE) build; \
	fi
	@echo "==> Running application..."
	@cd .. && ./$(TARGET_PATH)


.PHONY: debug
debug:
	@if command -v cargo >/dev/null 2>&1; then \
		echo "==> Found Rust toolchain, building natively..."; \
		cd .. && DEBUG=true RUST_LOG=debug cargo run --bin $(BINARY_NAME); \
	else \
		echo "==> Rust toolchain not found, building with Docker (Image: $(RUST_DOCKER_IMAGE))..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-v ../:/usr/src/app:z \
			-w /usr/src/app \
			$(RUST_DOCKER_IMAGE) \
			DEBUG=true RUST_LOG=debug cargo run --bin $(BINARY_NAME); \
	fi


.PHONY: test
test:
	@echo "==> Running archiver tests..."
	@cargo test

.PHONY: clean
clean:
	@echo "==> Cleaning up archiver build artifacts..."
	@cargo clean
	@rm -f ./$(BINARY_NAME).out


