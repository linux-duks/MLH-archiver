# Include shared container runtime detection
include ../containers.mk

# Define the Docker image to use for building
PYTHON_DOCKER_IMAGE = ghcr.io/astral-sh/uv:python3.14-trixie-slim


.PHONY: test
test:
	@if command -v nox >/dev/null 2>&1; then \
		echo "==> Found Python Testing toolchain, running natively..."; \
		nox; \
	else \
		echo "==> Python Testing toolchain not found, running with Docker (Image: $(PYTHON_DOCKER_IMAGE))..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-v .:/usr/src/app:z \
			-w /usr/src/app \
			$(PYTHON_DOCKER_IMAGE) \
			bash -c "uv sync --locked && uvx nox"; \
	fi


.PHONY: run
run:
	@if [ ! -d "../parser_output/parsed" ] || [ -z "$$(ls ../parser_output/parsed 2>/dev/null)" ]; then \
		echo "==> Error: '../parser_output/parsed' directory is missing or empty."; \
		echo "==> Please run the parser first to generate the dataset."; \
		exit 1; \
	else \
		echo "==> Running anonymizer..."; \
		$(COMPOSE) up && $(COMPOSE) down -v; \
	fi


.PHONY: rebuild
rebuild:
	$(COMPOSE) build --no-cache


.PHONY: debug
debug:
	INPUT_DIR="../parser_output/parsed" OUTPUT_DIR="../anonymizer_output" uv run src/main.py


.PHONY: clean
clean:
	@echo "==> Cleaning up anonymizer artifacts..."
	@rm -rf .venv
	@rm -rf .nox
	@rm -rf .ruff_cache
	@rm -rf .pytest_cache
	@rm -rf __pycache__
	@rm -rf **/__pycache__
