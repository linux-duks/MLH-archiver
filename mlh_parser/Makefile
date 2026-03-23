# if Podman is available, prefer using it over docker
ifeq ($(shell command -v podman 2> /dev/null),)
    CONTAINER=docker
else
    CONTAINER=podman
endif


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
	@if [ ! -d "../output" ] || [ -z "$$(ls ../output 2>/dev/null)" ]; then \
		echo "==> Error: 'output' directory is missing or empty."; \
		echo "==> Please run the archiver first to generate files."; \
		exit 1; \
	else \
		echo "==> Running parser..."; \
		$(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi


.PHONY: rebuild
rebuild:
	$(CONTAINER)-compose build


.PHONY: debug
debug:
	INPUT_DIR="../output" OUTPUT_DIR="../parser_output" uv run src/main.py


.PHONY: clean
clean:
	@echo "==> Cleaning up parser artifacts..."
	@rm -rf .venv
	@rm -rf .nox
	@rm -rf .ruff_cache
	@rm -rf .pytest_cache
	@rm -rf __pycache__
	@rm -rf **/__pycache__
