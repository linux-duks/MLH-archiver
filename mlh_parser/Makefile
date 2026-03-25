# if Podman is available, prefer using it over docker
ifeq ($(shell command -v podman 2> /dev/null),)
    CONTAINER=docker
else
    CONTAINER=podman
endif


# Define the Docker image to use for building
PYTHON_DOCKER_IMAGE = ghcr.io/astral-sh/uv:python3.14-trixie-slim

# Parser configuration variables (can be overridden from command line)
# Usage: make parse N_PROC=4 LISTS_TO_PARSE="list1,list2" REDO_FAILED_PARSES=true
N_PROC ?=
REDO_FAILED_PARSES ?=
LISTS_TO_PARSE ?=

# Build environment variables for parser
PARSER_ENV =
ifneq ($(N_PROC),)
    PARSER_ENV += N_PROC=$(N_PROC)
endif
ifneq ($(REDO_FAILED_PARSES),)
    PARSER_ENV += REDO_FAILED_PARSES=$(REDO_FAILED_PARSES)
endif
ifneq ($(LISTS_TO_PARSE),)
    PARSER_ENV += LISTS_TO_PARSE=$(LISTS_TO_PARSE)
endif


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
		if [ -n "$(PARSER_ENV)" ]; then \
			echo "==> With environment: $(PARSER_ENV)"; \
		fi; \
		$(PARSER_ENV) $(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi


.PHONY: rebuild
rebuild:
	GIT_COMMIT=$$(git rev-parse HEAD) \
	GIT_DATE=$$(git log -1 --format=%ci) \
	$(CONTAINER)-compose build --no-cache


.PHONY: debug
debug:
	$(PARSER_ENV) DEBUG=true INPUT_DIR="../output" OUTPUT_DIR="../parser_output" uv run src/main.py


.PHONY: clean
clean:
	@echo "==> Cleaning up parser artifacts..."
	@rm -rf .venv
	@rm -rf .nox
	@rm -rf .ruff_cache
	@rm -rf .pytest_cache
	@rm -rf __pycache__
	@rm -rf **/__pycache__
