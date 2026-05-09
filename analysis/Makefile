# Include shared container runtime detection
include ../containers.mk

# Define the Docker image to use for building
PYTHON_DOCKER_IMAGE="ghcr.io/astral-sh/uv:python3.14-trixie-slim"


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
			-e UV_CACHE_DIR=/tmp/.cache/uv \
			-v .:/usr/src/app:z \
			-w /usr/src/app \
			$(PYTHON_DOCKER_IMAGE) \
			bash -c "uv sync --locked && uvx nox"; \
	fi

.PHONY: run
run:
	@if [ ! -d "../parser_output/dataset" ] || [ -z "$$(ls ../parser_output/dataset 2>/dev/null)" ]; then \
		echo "==> Error: '../parser_output/dataset' directory is missing or empty."; \
		echo "==> Please run the parser first to generate the dataset."; \
		exit 1; \
	else \
		if command -v uv >/dev/null 2>&1; then \
			echo "==> Found uv toolchain, running natively..."; \
			INPUT_DIR="../parser_output/dataset,../anonymizer_output/" OUTPUT_DIR="../analysis/results/" uv run src/main.py; \
		else \
			echo "==> uv toolchain not found, running with Docker (Image: $(PYTHON_DOCKER_IMAGE))..."; \
			$(CONTAINER) run --rm \
				-it -u $$(id -u):$$(id -g) \
				--network=host \
				-e INPUT_DIR=/data/dataset,/data/anonymized \
				-e OUTPUT_DIR=./results/ \
				-e GIT_COMMIT=$${GIT_COMMIT:-unknown} \
				-e GIT_DATE=$${GIT_DATE:-unknown} \
				-e UV_CACHE_DIR=/tmp/.cache/uv \
				-v ../parser_output/dataset:/data/dataset:z \
				-v ../anonymizer_output/:/data/anonymized:z \
				-v .:/usr/src/app:z \
				-w /usr/src/app \
				$(PYTHON_DOCKER_IMAGE) \
				bash -c "uv sync --locked && uv run src/main.py"; \
		fi \
	fi

.PHONY: debug
debug:
	@if command -v uv >/dev/null 2>&1; then \
		echo "==> Found uv toolchain, running natively..."; \
		INPUT_DIR="../parser_output/dataset,../anonymizer_output/" OUTPUT_DIR="../analysis/results/" uv run src/make_analysis.py; \
	else \
		echo "==> uv toolchain not found, running with Docker..."; \
		$(CONTAINER) run --rm \
			-it -u $(id -u):$(id -g) \
			--network=host \
			-e INPUT_DIR=/data/dataset,/data/anonymized \
			-e OUTPUT_DIR=./results/ \
			-e UV_CACHE_DIR=/tmp/.cache/uv \
			-v ../parser_output/dataset:/data/dataset:z \
			-v ../anonymizer_output/:/data/anonymized:z \
			-v .:/usr/src/app:z \
			-w /usr/src/app \
			$(PYTHON_DOCKER_IMAGE) \
			bash -c "uv sync --locked && uv run src/make_analysis.py"; \
	fi

.PHONY: rebuild
rebuild:
	$(CONTAINER) pull $(PYTHON_DOCKER_IMAGE)
