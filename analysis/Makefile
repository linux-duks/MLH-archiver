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
	INPUT_DIR="../parser_output/parsed" uv run src/make_analysis.py
