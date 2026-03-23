# if Podman is available, prefer using it over docker
ifeq ($(shell command -v podman 2> /dev/null),)
    CONTAINER=docker
else
    CONTAINER=podman
endif

.PHONY: run
run:
	@if [ ! -d "../anonymizer_output" ] || [ -z "$$(ls ../anonymizer_output 2>/dev/null)" ]; then \
		echo "==> Error: '../anonymizer_output' directory is missing or empty."; \
		echo "==> Please run the anonymizer first to generate the dataset."; \
		exit 1; \
	else \
		echo "==> Running analysis..."; \
		$(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi

.PHONY: rebuild
rebuild:
	$(CONTAINER)-compose build --no-cache

.PHONY: debug
debug:
	INPUT_DIR="../parser_output/parsed" uv run src/make_analysis.py
