# Include shared container runtime detection
include ../containers.mk

.PHONY: run
run:
	@if [ ! -d "../anonymizer_output" ] || [ -z "$$(ls ../anonymizer_output 2>/dev/null)" ]; then \
		echo "==> Error: '../anonymizer_output' directory is missing or empty."; \
		echo "==> Please run the anonymizer first to generate the dataset."; \
		exit 1; \
	else \
		echo "==> Running analysis..."; \
		$(COMPOSE) up && $(COMPOSE) down -v; \
	fi

.PHONY: rebuild
rebuild:
	$(COMPOSE) build --no-cache

.PHONY: debug
debug:
	@if command -v uv >/dev/null 2>&1; then \
		echo "==> Found uv toolchain, running natively..."; \
		INPUT_DIR="../parser_output/parsed" uv run src/make_analysis.py; \
	else \
		echo "==> uv toolchain not found, running with Docker..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-e INPUT_DIR=/data/parsed \
			-v ../parser_output/parsed:/data/parsed:z \
			-v .:/usr/src/app:z \
			-w /usr/src/app \
			ghcr.io/astral-sh/uv:python3.14-trixie-slim \
			bash -c "uv sync --locked && uv run src/make_analysis.py"; \
	fi
