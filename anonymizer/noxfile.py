import nox
import os
import tomllib

def get_project_name():
    with open("pyproject.toml", "rb") as f:
        data = tomllib.load(f)
    return data["project"]["name"]

@nox.session(reuse_venv=True, venv_backend="uv")
def tests(session):
    """Run tests with coverage reporting."""
    session.install("pytest", "freezegun", "pytest-cov")
    session.install(".")

    # Generate coverage reports (HTML for local, markdown for CI)
    cov_args = [
        "--cov=src",
        "--cov-report=html:htmlcov",
        "--cov-report=term-missing",
    ]


    # Also write to a file for PR comments
    cov_args.append("--cov-report=markdown:coverage-summary.md")

    session.run("pytest", "-vv", *cov_args)

    cov_args.append("--cov-report=markdown:coverage-summary.md")

    session.run("pytest", "-vv", *cov_args)
    
    # Add header to the markdown file for PR comments
    if os.path.exists("coverage-summary.md"):
        with open("coverage-summary.md", "r") as f:
            content = f.read()
        with open("coverage-summary.md", "w") as f:
            f.write(f"## Python Coverage Summary : {get_project_name()} \n\n")
            f.write(content)


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
