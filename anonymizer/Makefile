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
