"""MLH Anonymizer - Entry point.

Pseudo-anonymize personal identification data in mailing list datasets.
"""

import os
import logging
from multiprocessing import Pool
import subprocess

from mlh_anonymizer.configs import (
    N_PROC,
    LISTS_TO_PARSE,
    DEBUG,
    INPUT_DIR_PATH,
    OUTPUT_DIR_PATH,
)
from mlh_anonymizer.list_processor import parse_mail_at

# Configure logging
level = logging.INFO
if DEBUG:
    level = logging.DEBUG

logging.basicConfig(
    level=level,
    format="[%(asctime)s] {%(pathname)s:%(lineno)d} %(levelname)s - %(message)s",
    datefmt="%H:%M:%S",
)

logger = logging.getLogger(__name__)


def parse_mail_at_wrap(mailing_list: str) -> None:
    """Wrapper for parse_mail_at with fixed paths."""
    parse_mail_at(mailing_list, INPUT_DIR_PATH, OUTPUT_DIR_PATH)


def main() -> None:
    logging.info("anonymizer starting — build: %s", _get_build_info())
    """Main entry point for the anonymizer."""
    # Parse specific lists or all in the directory
    lists = LISTS_TO_PARSE if len(LISTS_TO_PARSE) > 0 else os.listdir(INPUT_DIR_PATH)

    if N_PROC == 1:
        sequential(lists)
    else:
        with Pool(N_PROC) as p:
            try:
                p.map(parse_mail_at_wrap, lists)
            except KeyboardInterrupt:
                logging.info("Interrupted, shutting down workers...")
                p.terminate()
                p.join()


def _get_build_info() -> str:
    """Get build commit info: either from container build-time env, or from local git."""
    commit = os.getenv("BUILD_GIT_COMMIT")
    date = os.getenv("BUILD_GIT_DATE")

    # Prefer build-time info if set (inside container)
    if commit and commit != "unknown":
        return f"commit {commit} ({date})"

    # Fall back to local git (outside container)
    try:
        commit = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=os.path.dirname(os.path.abspath(__file__)),
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        date = subprocess.check_output(
            ["git", "log", "-1", "--format=%ci"],
            cwd=os.path.dirname(os.path.abspath(__file__)),
            text=True,
            stderr=subprocess.DEVNULL,
        ).strip()
        return f"commit {commit} ({date})"
    except Exception:
        return "unknown"


def sequential(lists: list) -> None:
    """Run anonymization sequentially (for debugging).

    Args:
        lists: List of mailing list names to process
    """
    for mailing_list in lists:
        parse_mail_at_wrap(mailing_list)


if __name__ == "__main__":
    main()
