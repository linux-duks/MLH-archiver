# if Podman is available, prefer using it over docker
ifeq ($(shell command -v podman 2> /dev/null),)
    CONTAINER=docker
else
    CONTAINER=podman
endif

.PHONY: run
run:
	@if [ ! -d "../parser_output/parsed" ] || [ -z "$$(ls ../parser_output/parsed 2>/dev/null)" ]; then \
		echo "==> Error: '../parser_output/parsed' directory is missing or empty."; \
		echo "==> Please run the parser first to generate the dataset."; \
		exit 1; \
	else \
		echo "==> Running analysis..."; \
		$(CONTAINER)-compose up && $(CONTAINER)-compose down -v; \
	fi

.PHONY: rebuild
rebuild:
	$(CONTAINER)-compose build

.PHONY: debug
debug:
	INPUT_DIR="../parser_output/parsed" uv run src/make_analysis.py
