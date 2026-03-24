from multiprocessing import Pool
import os
import logging
import subprocess

from mlh_parser.parser import parse_mail_at
from mlh_parser.configs import (
    N_PROC,
    LISTS_TO_PARSE,
    DEBUG,
    INPUT_DIR_PATH,
    OUTPUT_DIR_PATH,
    FAIL_ON_PARSING_ERROR,
)

level = logging.INFO
if DEBUG:
    level = logging.DEBUG

logging.basicConfig(
    level=level,
    format="[%(asctime)s] {%(pathname)s:%(lineno)d} %(levelname)s - %(message)s",
    datefmt="%H:%M:%S",
)


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


def parse_mail_at_wrap(mail_l):
    return parse_mail_at(mail_l, INPUT_DIR_PATH, OUTPUT_DIR_PATH, FAIL_ON_PARSING_ERROR)


def main():
    logging.info("mlh_parser starting — build: %s", _get_build_info())

    # parse specific lists or all in the directory
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


# for debugging only
def sequential(lists: list):
    for mail_l in lists:
        parse_mail_at_wrap(mail_l)


if __name__ == "__main__":
    main()
