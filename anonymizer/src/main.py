"""MLH Anonymizer - Entry point.

Pseudo-anonymize personal identification data in mailing list datasets.
"""

import os
import logging
from multiprocessing import Pool

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


def sequential(lists: list) -> None:
    """Run anonymization sequentially (for debugging).

    Args:
        lists: List of mailing list names to process
    """
    for mailing_list in lists:
        parse_mail_at_wrap(mailing_list)


if __name__ == "__main__":
    main()
