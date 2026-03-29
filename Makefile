# By default, 'make' will run the 'all' target
.PHONY: all
all: rebuild run


# run all targets in order
.PHONY: run
run: run-archiver parse anonymize analysis

# ------------------------------------------------------------------------------
# APPLICATION TARGETS
# ------------------------------------------------------------------------------

.PHONY: build-archiver
 build-archiver:
	$(MAKE) -C mlh-archiver build

.PHONY: run-archiver
run-archiver:
	$(MAKE) -C mlh-archiver run


.PHONY: debug-archiver
debug-archiver:
	$(MAKE) -C mlh-archiver debug

.PHONY: parse
parse:
	$(MAKE) -C mlh_parser run N_PROC="$(N_PROC)" REDO_FAILED_PARSES="$(REDO_FAILED_PARSES)" LISTS_TO_PARSE="$(LISTS_TO_PARSE)"
.PHONY: anonymize
anonymize:
	$(MAKE) -C anonymizer run

.PHONY: analysis
analysis:
	$(MAKE) -C analysis run

# ------------------------------------------------------------------------------
# REBUILD TARGETS
# ------------------------------------------------------------------------------

.PHONY: rebuild
rebuild: rebuild-parser rebuild-anonymizer rebuild-analysis build-archiver

.PHONY: rebuild-parser
rebuild-parser:
	$(MAKE) -C mlh_parser rebuild

.PHONY: rebuild-anonymizer
rebuild-anonymizer:
	$(MAKE) -C anonymizer rebuild

.PHONY: rebuild-analysis
rebuild-analysis:
	$(MAKE) -C analysis rebuild

# ------------------------------------------------------------------------------
# DEBUG TARGETS
# ------------------------------------------------------------------------------

.PHONY: debug-parser
debug-parser:
	$(MAKE) -C mlh_parser debug N_PROC="$(N_PROC)" REDO_FAILED_PARSES="$(REDO_FAILED_PARSES)" LISTS_TO_PARSE="$(LISTS_TO_PARSE)"

.PHONY: debug-anonymizer
debug-anonymizer:
	$(MAKE) -C anonymizer debug

# ------------------------------------------------------------------------------
# TEST TARGETS
# ------------------------------------------------------------------------------

.PHONY: test
test: test-archiver test-parser test-anonymizer

.PHONY: test-archiver
test-archiver:
	$(MAKE) -C mlh-archiver test

.PHONY: test-parser
test-parser:
	$(MAKE) -C mlh_parser test

.PHONY: test-anonymizer
test-anonymizer:
	$(MAKE) -C anonymizer test

# ------------------------------------------------------------------------------
# UTILITY TARGETS
# ------------------------------------------------------------------------------

.PHONY: clean
clean:
	@echo "==> Cleaning up build artifacts..."
	$(MAKE) -C mlh_parser clean
	$(MAKE) -C anonymizer clean
	$(MAKE) -C analysis clean
	$(MAKE) -C scripts clean
	$(MAKE) -C mlh-archiver clean

