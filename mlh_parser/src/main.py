from multiprocessing import Pool
import os
import logging

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
if DEBUG != "false":
    level = logging.DEBUG

logging.basicConfig(
    level=level,
    format="[%(asctime)s] {%(pathname)s:%(lineno)d} %(levelname)s - %(message)s",
    datefmt="%H:%M:%S",
)


def parse_mail_at_wrap(mail_l):
    return parse_mail_at(mail_l, INPUT_DIR_PATH, OUTPUT_DIR_PATH, FAIL_ON_PARSING_ERROR)


def main():
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
