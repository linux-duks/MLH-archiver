import multiprocessing
import os

DEBUG = os.getenv("DEBUG", False)
N_PROC = os.getenv("N_PROC", multiprocessing.cpu_count() / 2 if not DEBUG else 1)
if DEBUG:
    print(f"Running in DEBUG mode. N_PROC {N_PROC}")

REDO_FAILED_PARSES = os.getenv(
    "REDO_FAILED_PARSES", False
)  # Parse only the emails that were unsuccessfully parsed on previous runs.


LISTS_TO_PARSE = [item for item in os.getenv("LISTS_TO_PARSE", "").split(",") if item]
