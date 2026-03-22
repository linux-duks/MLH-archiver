import nox


@nox.session(reuse_venv=True, venv_backend="uv")
def tests(session):
    session.install("pytest", "freezegun")
    session.install(".")
    session.run("pytest", "-vv")


@nox.session(reuse_venv=True, venv_backend="uv")
def format(session):
    session.install("ruff")
    session.run("ruff", "format", ".")
