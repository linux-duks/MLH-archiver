import io
import os
import re
import traceback
import polars as pl
from mlh_parser.date_parser import process_date
import logging

from mlh_parser.parser_algorithm import parse_email_bytes_to_dict
from mlh_parser.configs import REDO_FAILED_PARSES, PARQUET_FILE_NAME
from mlh_parser.constants import (
    PARQUET_COLS_SCHEMA,
    SINGLE_VALUED_COLS,
)

logger = logging.getLogger(__name__)


def parse_mail_at(mailing_list, input_dir_path, output_dir_path, fail_on_parsing_error):
    """
    Parses the emails from a single specified list,
    to be found in INPUT_DIR_PATH/mailing_list
    """

    list_input_path = input_dir_path + "/" + mailing_list
    list_output_path = output_dir_path + "/" + mailing_list
    parquet_dir_path = output_dir_path + "/parsed"
    success_output_path = parquet_dir_path + "/list=" + mailing_list
    parquet_path = success_output_path + "/" + PARQUET_FILE_NAME
    error_output_path = list_output_path + "/errors"

    all_parsed = pl.DataFrame(schema=PARQUET_COLS_SCHEMA)

    if not os.path.isdir(list_output_path):
        logger.info(f"First parse of list '{mailing_list}'")

        # Use makedirs with exist_ok=True to safely handle concurrent workers
        # all trying to create the same parquet_dir_path simultaneously
        os.makedirs(parquet_dir_path, exist_ok=True)
        os.makedirs(list_output_path, exist_ok=True)
        os.makedirs(success_output_path, exist_ok=True)
        os.makedirs(error_output_path, exist_ok=True)
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
        email_files = (
            f for f in all_emails if os.path.isfile(os.path.join(error_output_path, f))
        )
        input_dir_for_files = error_output_path
    else:
        all_emails = os.listdir(list_input_path)
        email_files = (
            f
            for f in all_emails
            if f
            not in (
                "__progress.yaml",
                "errors.md",
                "__errors.csv",
                "errors.txt",
            )
            and os.path.isfile(os.path.join(list_input_path, f))
        )
        input_dir_for_files = list_input_path

    def _yield_email_items(file_path):
        """Yield (file_name, content) tuples from an .eml or .parquet file."""
        _, ext = os.path.splitext(file_path)
        file_name = os.path.basename(file_path)
        if ext == ".eml":
            with open(file_path, mode="r", encoding="utf-8") as f:
                yield (file_name, f.read())
        elif ext == ".parquet":
            df = pl.read_parquet(file_path)
            for row in df.iter_rows(named=True):
                yield (row["email_id"] + ":" + file_name, row["content"])

    def email_name_iterator():
        """Iterate over all files, yielding (file_name, content) from each."""
        for email_file in email_files:
            full_path = os.path.join(input_dir_for_files, email_file)
            yield from _yield_email_items(full_path)

    email_dict_list = []
    error_files_to_delete = []

    for email_name, email_content in email_name_iterator():
        ctx = {
            "file_name": email_name,
            "mailing_list": mailing_list,
            "errors": [],
        }

        email_file = io.StringIO(email_content)
        email_file_bytes = io.BytesIO(email_content.encode("utf-8"))

        try:
            email_as_dict = parse_email_bytes_to_dict(email_file_bytes.read(), ctx=ctx)
            email_as_dict = post_process_parsed_mail(email_as_dict, ctx=ctx)
        except Exception as parsing_error:
            ctx["errors"].append(f"{type(parsing_error).__name__}: {parsing_error}")
            save_unsuccessful_parse(
                email_file,
                parsing_error,
                email_name,
                mailing_list,
                error_output_path,
                email_file_bytes=email_file_bytes,
                ctx=ctx,
            )
            if fail_on_parsing_error:
                raise parsing_error
            else:
                continue

        email_as_dict = sanitize_surrogate_characters(email_as_dict)
        email_as_dict["__file_name"] = email_name
        email_dict_list.append(email_as_dict)

        if REDO_FAILED_PARSES:
            error_files_to_delete.append(error_output_path + "/" + email_name)

        email_file.close()
        email_file_bytes.close()

    newly_parsed = pl.DataFrame(
        email_dict_list, schema=PARQUET_COLS_SCHEMA
    ).with_row_index()

    print(f"Converting {mailing_list} to Polars DataFrame")

    all_parsed.extend(newly_parsed)
    all_parsed = all_parsed.drop("index")
    all_parsed.write_parquet(parquet_path)

    if REDO_FAILED_PARSES:
        for error_file in error_files_to_delete:
            os.remove(error_file)

    logger.info(f"Saved all parsed mail on list '{mailing_list}'")


def sanitize_surrogate_characters(email_as_dict: dict) -> dict:
    # Sanitize surrogate characters from all string fields to prevent
    # UnicodeEncodeError when creating Polars DataFrame
    if email_as_dict is None:
        return None
    for key, value in email_as_dict.items():
        if isinstance(value, str):
            email_as_dict[key] = value.encode("utf-8", errors="surrogatepass").decode(
                "utf-8", errors="replace"
            )
        elif isinstance(value, list):
            for j, item in enumerate(value):
                if isinstance(item, str):
                    value[j] = item.encode("utf-8", errors="surrogatepass").decode(
                        "utf-8", errors="replace"
                    )
                elif isinstance(item, dict):
                    for k, v in item.items():
                        if isinstance(v, str):
                            item[k] = v.encode("utf-8", errors="surrogatepass").decode(
                                "utf-8", errors="replace"
                            )
    return email_as_dict


def post_process_parsed_mail(email_as_dict: dict, ctx: dict = None):
    """
    Post-processes dict containing email fields, parsing
    multiple valued fields and other non Str fields.
    Ensures all required fields are present with defaults.

    Args:
        email_as_dict: Parsed email data
        ctx: Context dict with file_name, mailing_list, errors
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    # Handle list fields (default to empty list if missing)
    if "to" not in email_as_dict or email_as_dict["to"] is None:
        email_as_dict["to"] = []
    elif isinstance(email_as_dict["to"], str):
        email_as_dict["to"] = [
            x.strip() for x in email_as_dict["to"].split(",") if x.strip()
        ]

    if "cc" not in email_as_dict or email_as_dict["cc"] is None:
        email_as_dict["cc"] = []
    elif isinstance(email_as_dict["cc"], str):
        email_as_dict["cc"] = [
            x.strip() for x in email_as_dict["cc"].split(",") if x.strip()
        ]

    if "references" not in email_as_dict or email_as_dict["references"] is None:
        email_as_dict["references"] = []
    elif isinstance(email_as_dict["references"], str):
        email_as_dict["references"] = email_as_dict["references"].split()

    if "trailers" not in email_as_dict or email_as_dict["trailers"] is None:
        email_as_dict["trailers"] = []

    if "code" not in email_as_dict or email_as_dict["code"] is None:
        email_as_dict["code"] = []

    # Handle single-value string fields (default to empty string if missing)
    for column in SINGLE_VALUED_COLS:
        if column not in email_as_dict or email_as_dict[column] is None:
            if column == "date":
                email_as_dict[column] = None  # date can be None
            else:
                email_as_dict[column] = ""
        elif isinstance(email_as_dict[column], list):
            email_as_dict[column] = (
                email_as_dict[column][0] if email_as_dict[column] else ""
            )

    email_as_dict = process_date(email_as_dict)

    return email_as_dict


def parse_and_process_email(email_file_data: bytes, ctx: dict = None) -> dict:
    """
    Run parse_email_txt_to_dict and post_process_parsed_mail
    Post-processes dict containing email fields, parsing
    multiple valued fields and other non Str fields.
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    email_as_dict = parse_email_bytes_to_dict(email_file_data, ctx=ctx)

    return post_process_parsed_mail(email_as_dict, ctx=ctx)


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
    email_file,
    parsing_error,
    email_name,
    mailing_list,
    error_output_path,
    email_file_bytes=None,
    ctx: dict = None,
):
    """
    Saves information on unsuccessful email parse. Both original email content and
    exception information are stored in the directory at <error_output_path>, in
    a file with the same name as the original .eml file.

    Args:
        email_file: File handle for the email
        parsing_error: Exception that was raised
        email_name: Name of the email file
        mailing_list: Name of the mailing list
        error_output_path: Path to save error files
        email_file_bytes: Binary file handle
        ctx: Context dict with file_name, mailing_list, errors
    """
    if ctx is None:
        ctx = {"file_name": email_name, "mailing_list": mailing_list, "errors": []}

    # Build detailed error information
    error_details = [
        "=" * 60,
        f"PARSE ERROR: {ctx['file_name']}",
        f"Mailing list: {ctx['mailing_list']}",
        f"Error type: {type(parsing_error).__name__}",
        f"Error message: {parsing_error}",
        "=" * 60,
        "",
        "Traceback:",
        traceback.format_exc(),
        "",
        "=" * 60,
        "EMAIL HEADERS (first 50 lines):",
        "=" * 60,
    ]

    # Add email headers preview from text file if available
    if email_file is not None:
        try:
            email_file.seek(0, os.SEEK_SET)
            header_lines = []
            for i, line in enumerate(email_file.readlines()):
                if i >= 50:
                    header_lines.append("... (truncated)")
                    break
                header_lines.append(line.rstrip())
                # Stop at end of headers (empty line)
                if line.strip() == "" and i > 0:
                    break

            error_details.extend(header_lines)
            email_file.close()
        except Exception:
            error_details.append("(Could not read email headers)")

    to_save = "\n".join(error_details)

    # Log with full traceback and context
    logger.error(
        f"[{ctx['mailing_list']}/{ctx['file_name']}] "
        f"{type(parsing_error).__name__}: {parsing_error}"
    )
    logger.debug(f"Full traceback:\n{traceback.format_exc()}")

    with open(
        error_output_path + "/" + email_name, "w", encoding="utf-8"
    ) as error_output_file:
        error_output_file.write(to_save)

    # Close binary file handle if provided
    if email_file_bytes is not None:
        email_file_bytes.close()


def remove_previous_errors(errors_dir_path):
    """
    Removes every file from the directory at the path given.
    """

    for error_file_name in os.listdir(errors_dir_path):
        os.remove(errors_dir_path + "/" + error_file_name)
