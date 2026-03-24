import multiprocessing
import os
import math

DEBUG = os.getenv("DEBUG", False)
N_PROC = os.getenv("N_PROC")
N_PROC = (
    int(N_PROC) if N_PROC.isdecimal() else math.ceil(multiprocessing.cpu_count() / 3)
)

if DEBUG:
    N_PROC = 1
    print(f"Running in DEBUG mode. N_PROC {N_PROC}")

REDO_FAILED_PARSES = os.getenv(
    "REDO_FAILED_PARSES", False
)  # Parse only the emails that were unsuccessfully parsed on previous runs.


LISTS_TO_PARSE = [item for item in os.getenv("LISTS_TO_PARSE", "").split(",") if item]
