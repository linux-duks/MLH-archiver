import nox


@nox.session(reuse_venv=True, venv_backend="uv")
def tests(session):
    """Run tests with coverage reporting."""
    session.install("pytest", "freezegun", "pytest-cov")
    session.install(".")

    # Generate coverage reports (XML for CI, HTML for local dev)
    session.run(
        "pytest",
        "-vv",
        "--cov=src",
        "--cov-report=xml:coverage.xml",
        "--cov-report=html:htmlcov",
        "--cov-report=term-missing",
    )


@nox.session(reuse_venv=True, venv_backend="uv")
def coverage(session):
    """Display coverage report from existing data."""
    session.install("coverage[toml]")
    session.run("coverage", "report", "--show-missing")
    session.run("coverage", "html")


@nox.session(reuse_venv=True, venv_backend="uv")
def format(session):
    session.install("ruff")
    session.run("ruff", "format", ".")


@nox.session(reuse_venv=True, venv_backend="uv")
def lint(session):
    session.install("ruff")
    session.run("ruff", "check", ".")
