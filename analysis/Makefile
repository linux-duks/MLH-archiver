# Detect container runtime and compose command
# Priority: podman > docker compose (v2) > docker-compose (v1)
ifeq ($(shell command -v podman 2> /dev/null),)
    CONTAINER=docker
    # Check for docker compose (v2 plugin) first, then docker-compose (v1 standalone)
    ifeq ($(shell docker compose version 2> /dev/null),)
        COMPOSE=docker-compose
    else
        COMPOSE=docker compose
    endif
else
    CONTAINER=podman
    COMPOSE=podman-compose
endif

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
	INPUT_DIR="../parser_output/parsed" uv run src/make_analysis.py
