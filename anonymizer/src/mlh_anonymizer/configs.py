"""Configuration and environment variable handling for the anonymizer."""

import os
import multiprocessing
import math

from mlh_anonymizer.constants import N_PROC_DEFAULT_MAX


def _parse_n_proc() -> int:
    """Parse N_PROC from environment variable.

    Returns:
        Number of processes to use
    """
    n_proc_env = os.getenv("N_PROC", "")
    if n_proc_env.isdecimal():
        return int(n_proc_env)
    return max(math.ceil(multiprocessing.cpu_count() / 3), N_PROC_DEFAULT_MAX)


def _is_debug() -> bool:
    """Check if debug mode is enabled.

    Returns:
        True if DEBUG environment variable is set to "true"
    """
    return os.getenv("DEBUG", "false").lower() == "true"


# Runtime configuration
DEBUG: bool = _is_debug()
N_PROC: int = _parse_n_proc()

# Override N_PROC for debug mode
if DEBUG:
    N_PROC = 1
    print(f"Running in DEBUG mode. N_PROC {N_PROC}")

# List of specific mailing lists to parse (empty = parse all)
LISTS_TO_PARSE: list[str] = [
    item for item in os.getenv("LISTS_TO_PARSE", "").split(",") if item
]

# Directory paths (required environment variables)
INPUT_DIR_PATH: str = os.environ["INPUT_DIR"]
OUTPUT_DIR_PATH: str = os.environ["OUTPUT_DIR"]
