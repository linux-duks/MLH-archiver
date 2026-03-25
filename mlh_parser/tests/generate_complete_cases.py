#!/usr/bin/env python3
"""
Generate missing .pytest fixture files for .eml files in complete_cases/.

For each .eml that is missing one or more of:
  .body.pytest   – email body as returned by get_body()
  .headers.pytest – email headers in key: value format
  .trailers.pytest – parsed trailers as a Python list literal
  .code.pytest   – extracted patches as a Python list literal

…the script runs the parser and writes the missing files.

Usage (from mlh_parser/ directory):
    uv run python tests/generate_complete_cases.py
    uv run python tests/generate_complete_cases.py --dry-run
    uv run python tests/generate_complete_cases.py --all   # regenerate all, even if present
"""

import argparse
import pprint
import sys
from pathlib import Path

# Make mlh_parser importable when run directly
sys.path.insert(0, str(Path(__file__).parent.parent / "src"))

from mlh_parser.email_reader import decode_mail, get_body, get_headers
from mlh_parser.parser import parse_and_process_email

COMPLETE_CASES_DIR = Path(__file__).parent / "complete_cases"
FIXTURE_EXTENSIONS = [
    ".body.pytest",
    ".headers.pytest",
    ".trailers.pytest",
    ".code.pytest",
]

# Suffixes that mark a fixture file (e.g. .body, .headers) — used to detect
# files like "foo.body.eml" that are misnamed fixtures, not real emails.
_FIXTURE_STEMS = {ext.replace(".pytest", "") for ext in FIXTURE_EXTENSIONS}


# ---------------------------------------------------------------------------
# Formatters
# ---------------------------------------------------------------------------


def _write_headers(headers: dict) -> str:
    """Serialise headers dict back to key: value lines for .headers.pytest."""
    lines = []
    for key, value in headers.items():
        if isinstance(value, list):
            for v in value:
                lines.append(f"{key}: {v}")
        else:
            lines.append(f"{key}: {value}")
    return "\n".join(lines) + "\n"


def _write_python_literal(obj) -> str:
    """Serialise a Python object as a pretty-printed literal (eval-able)."""
    return pprint.pformat(obj, sort_dicts=False) + "\n"


# ---------------------------------------------------------------------------
# Core
# ---------------------------------------------------------------------------


def missing_fixtures(eml_path: Path) -> list[str]:
    """Return list of extension strings (e.g. '.body.pytest') that are absent."""
    stem = eml_path.stem
    return [
        ext
        for ext in FIXTURE_EXTENSIONS
        if not (eml_path.parent / (stem + ext)).exists()
    ]


def generate_fixtures(eml_path: Path, targets: list[str], dry_run: bool) -> None:
    """Parse eml_path and write the requested fixture files."""
    mail_bytes = eml_path.read_bytes()

    # Parse lazily — only call what we actually need
    msg = None
    headers = None
    body = None
    result = None

    def _ensure_msg():
        nonlocal msg
        if msg is None:
            msg = decode_mail(mail_bytes)

    def _ensure_headers():
        nonlocal headers
        _ensure_msg()
        if headers is None:
            headers = get_headers(msg, raw_email=mail_bytes)

    def _ensure_body():
        nonlocal body
        _ensure_msg()
        if body is None:
            body = get_body(msg)

    def _ensure_result():
        nonlocal result
        if result is None:
            result = parse_and_process_email(mail_bytes)

    for ext in targets:
        target = eml_path.parent / (eml_path.stem + ext)

        try:
            if ext == ".body.pytest":
                _ensure_body()
                content = body

            elif ext == ".headers.pytest":
                _ensure_headers()
                content = _write_headers(headers)

            elif ext == ".trailers.pytest":
                _ensure_result()
                content = _write_python_literal(result.get("trailers", []))

            elif ext == ".code.pytest":
                _ensure_result()
                content = _write_python_literal(result.get("code", []))

            else:
                print(f"  [SKIP] Unknown extension {ext}")
                continue

        except Exception as exc:
            print(f"  [ERROR] {target.name}: {exc}")
            continue

        if dry_run:
            print(f"  [dry-run] {target.name}")
        else:
            target.write_text(content, encoding="utf-8")
            print(f"  [created] {target.name}")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be created without writing anything",
    )
    parser.add_argument(
        "--all",
        action="store_true",
        help="Regenerate all fixture files, even those that already exist",
    )
    args = parser.parse_args()

    all_eml = sorted(COMPLETE_CASES_DIR.glob("*.eml"))
    eml_files = []
    for f in all_eml:
        # Skip files like "foo.body.eml" — those are misnamed fixture files.
        if any(f.stem.endswith(suffix) for suffix in _FIXTURE_STEMS):
            print(f"[warn] Skipping likely misnamed fixture: {f.name}")
            continue
        eml_files.append(f)

    if not eml_files:
        print(f"No .eml files found in {COMPLETE_CASES_DIR}")
        return

    work = []
    for eml in eml_files:
        targets = FIXTURE_EXTENSIONS if args.all else missing_fixtures(eml)
        if targets:
            work.append((eml, targets))

    if not work:
        print("All .eml files already have complete fixture sets.")
        return

    print(
        f"{'Would process' if args.dry_run else 'Processing'} "
        f"{len(work)} .eml file(s):\n"
    )

    for eml, targets in work:
        print(f"{eml.name}  →  {', '.join(targets)}")
        generate_fixtures(eml, targets, dry_run=args.dry_run)

    print("\nDone.")


if __name__ == "__main__":
    main()
