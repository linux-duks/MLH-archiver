import io
import os
import re
import polars as pl
from mlh_parser.date_parser import process_date
from tqdm import tqdm
import logging

from mlh_parser.parser_algorithm import parse_email_bytes_to_dict
from mlh_parser.constants import (
    PARQUET_COLS_SCHEMA,
    REDO_FAILED_PARSES,
    SINGLE_VALUED_COLS,
)

logger = logging.getLogger(__name__)


def parse_mail_at(mailing_list, input_dir_path, output_dir_path):
    """
    Parses the emails from a single specified list,
    to be found in INPUT_DIR_PATH/mailing_list
    """

    PARQUET_DIR_PATH = output_dir_path + "/parsed"
    PARQUET_FILE_NAME = "list_data.parquet"

    list_input_path = input_dir_path + "/" + mailing_list
    list_output_path = output_dir_path + "/" + mailing_list
    success_output_path = PARQUET_DIR_PATH + "/list=" + mailing_list
    parquet_path = success_output_path + "/" + PARQUET_FILE_NAME
    error_output_path = list_output_path + "/errors"

    all_parsed = pl.DataFrame(schema=PARQUET_COLS_SCHEMA)

    if not os.path.isdir(list_output_path):
        logger.info(f"First parse of list '{mailing_list}'")

        if not os.path.isdir(PARQUET_DIR_PATH):
            os.mkdir(PARQUET_DIR_PATH)

        os.mkdir(list_output_path)
        os.mkdir(success_output_path)
        os.mkdir(error_output_path)
    else:
        if REDO_FAILED_PARSES:
            try:
                all_parsed = pl.read_parquet(parquet_path)
            except FileNotFoundError:
                pass
        else:
            remove_previous_errors(error_output_path)
    all_parsed = all_parsed.with_row_index()

    if REDO_FAILED_PARSES:
        all_emails = os.listdir(error_output_path)
    else:
        all_emails = os.listdir(list_input_path)
        # remove metadata files
        all_emails.remove("__last_article_number")
        if "errors.md" in all_emails:
            all_emails.remove("errors.md")
        if "__errors" in all_emails:
            all_emails.remove("__errors")
        if "errors.txt" in all_emails:
            all_emails.remove("errors.txt")

    newly_parsed = pl.DataFrame(schema=PARQUET_COLS_SCHEMA)
    newly_parsed = newly_parsed.with_row_index()

    for email_name in tqdm(all_emails):
        email_path = list_input_path + "/" + email_name
        email_file = io.open(email_path, mode="r", encoding="utf-8")
        email_file_bytes = io.open(email_path, mode="rb")
        email_file_bytes = io.open(email_path, mode="rb")

        try:
            # email_as_dict = parse_email_txt_to_dict(email_file.read())
            email_as_dict = parse_email_bytes_to_dict(email_file_bytes.read())

            email_as_dict = post_process_parsed_mail(email_as_dict)
            # email_as_dict = parse_email_txt_to_dict(email_file.read())
            email_as_dict = parse_email_bytes_to_dict(email_file_bytes.read())

            email_as_dict = post_process_parsed_mail(email_as_dict)
        except Exception as parsing_error:
            save_unsuccessful_parse(
                email_file, parsing_error, email_name, mailing_list, error_output_path
            )
            continue

        email_as_df = pl.DataFrame(email_as_dict, schema=PARQUET_COLS_SCHEMA)
        email_as_df = email_as_df.with_columns(
            # Let's keep our datetimes naive
            pl.col("date").dt.replace_time_zone(None)
        )

        email_as_df = email_as_df.with_row_index()
        newly_parsed.extend(email_as_df)  # Simply adds to end of DF

        email_file.close()

        if REDO_FAILED_PARSES:
            os.remove(error_output_path + "/" + email_name)

    all_parsed.extend(newly_parsed)
    all_parsed = all_parsed.drop("index")
    all_parsed.write_parquet(parquet_path)
    logger.info(f"Saved all parsed mail on list '{mailing_list}'")


def post_process_parsed_mail(email_as_dict: dict):
    """
    Post-processes dict containing email fields, parsing
    multiple valued fields and other non Str fields.
    """

    if isinstance(email_as_dict["to"], str):
        email_as_dict["to"] = email_as_dict["to"].split(",")

    if isinstance(email_as_dict["cc"], str):
        email_as_dict["cc"] = email_as_dict["cc"].split(",")

    for column in SINGLE_VALUED_COLS:
        if isinstance(email_as_dict[column], list):
            email_as_dict[column] = email_as_dict[column][0]
            # This usually doesn't make sense
            # For dates, we're saving the first date parsed

    if isinstance(email_as_dict["references"], str):
        email_as_dict["references"] = email_as_dict["references"].split(" ")

    email_as_dict = process_date(email_as_dict)

    return email_as_dict


def parse_and_process_email(email_file_data: bytes) -> dict:
    """
    Run parse_email_txt_to_dict and post_process_parsed_mail
    Post-processes dict containing email fields, parsing
    multiple valued fields and other non Str fields.
    """
    email_as_dict = parse_email_bytes_to_dict(email_file_data)

    return post_process_parsed_mail(email_as_dict)


def get_email_id(email_file) -> str:
    """
    Retrieves the email Message-ID.
    """

    for line in email_file.readlines():
        if re.match(r"^Message-ID:", line, re.IGNORECASE):
            message_id = line[len("Message-ID:") :].strip()
            email_file.seek(0, os.SEEK_SET)
            return message_id

    email_file.seek(0, os.SEEK_SET)  # Return to the beginning of file stream

    raise Exception("Found email with no Message-ID field for file " + email_file.name)


def email_previously_parsed(all_parsed, email_id) -> int | None:
    """
    Checks whether the given email message id corresponds
    to a email saved in the archive. If that's the case,
    returns the dataframe row where the email is stored.
    Otherwise, returns None.
    """

    filter_res = all_parsed.filter(pl.col("message-id") == email_id)

    if filter_res.shape[0] == 0:
        return None
    elif filter_res.shape[0] > 1:
        raise Exception("Message-ID conflict on parquet database for id " + email_id)

    return filter_res[0, "index"]


def save_unsuccessful_parse(
    email_file, parsing_error, email_name, mailing_list, error_output_path
):
    """
    Saves information on unsuccessful email parse. Both original email content and
    exception information are stored in the directory at <error_output_path>, in
    a file with the same name as the original .eml file.
    """
    email_file.seek(0, os.SEEK_SET)  # Return to the beginning of file stream

    to_save = email_file.read()
    to_save += "\n" + "=" * 30 + " Exception:\n"
    to_save += str(parsing_error)

    logger.error(f"Error when parsing file '{email_name}' of list '{mailing_list}'")
    logger.error(parsing_error)

    with open(
        error_output_path + "/" + email_name, "w", encoding="utf-8"
    ) as error_output_file:
        error_output_file.write(to_save)

    email_file.close()


def remove_previous_errors(errors_dir_path):
    """
    Removes every file from the directory at the path given.
    """

    for error_file_name in os.listdir(errors_dir_path):
        os.remove(errors_dir_path + "/" + error_file_name)
