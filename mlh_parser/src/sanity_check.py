"""
Runs a sanity check on a set of parsed lists.

Checks whether every parquet file from a parsed list has every email
from the original list.
"""

import os
import polars as pl

from mlh_parser.configs import INPUT_DIR_PATH, PARQUET_DIR_PATH, PARQUET_FILE_NAME


def main():
    print("-" * 20, "Sanity Check!", "-" * 20)

    total_emails = 0
    total_parsed = 0

    for mailing_list in os.listdir(INPUT_DIR_PATH):
        num_emails = get_list_len(mailing_list)
        num_parquet = get_entries_in_list_parquet(mailing_list)
        error_msg = (
            ""
            if num_emails == num_parquet
            else str(num_emails - num_parquet) + " emails missing!!"
        )
        print(mailing_list, "|", num_emails, "|", num_parquet, "|", error_msg)

        total_emails += num_emails
        total_parsed += num_parquet

    print("-" * (42 + len("Sanity Check!")))
    print("Number of emails available:", total_emails)
    print("Number of emails in parquet files:", total_parsed)


def get_list_len(mailing_list):
    list_input_path = INPUT_DIR_PATH + "/" + mailing_list

    all_emails = os.listdir(list_input_path)
    all_emails.remove("__progress.yaml")
    if "errors.md" in all_emails:
        all_emails.remove("errors.md")
    if "__errors.csv" in all_emails:
        all_emails.remove("__errors.csv")
    if "errors.txt" in all_emails:
        all_emails.remove("errors.txt")

    return len(all_emails)


def get_entries_in_list_parquet(mailing_list):
    parquet_path = PARQUET_DIR_PATH + "/list=" + mailing_list + "/" + PARQUET_FILE_NAME
    df = pl.read_parquet(parquet_path)
    num_rows = len(df)
    del df
    return num_rows


if __name__ == "__main__":
    main()
