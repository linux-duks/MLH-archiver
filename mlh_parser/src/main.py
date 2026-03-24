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
    return parse_mail_at(mail_l, INPUT_DIR_PATH, OUTPUT_DIR_PATH)


def main():
    p = Pool(N_PROC)

    if len(LISTS_TO_PARSE) > 0:
        p.map(parse_mail_at_wrap, LISTS_TO_PARSE)
    else:
        p.map(parse_mail_at_wrap, os.listdir(INPUT_DIR_PATH))


# for debugging only
def sequential():
    for mail_l in os.listdir(INPUT_DIR_PATH):
        parse_mail_at_wrap(mail_l)


if __name__ == "__main__":
    main()
