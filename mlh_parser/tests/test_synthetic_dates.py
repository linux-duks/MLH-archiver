"""
Date parsing tests using synthetic .eml fixtures.

These cover edge cases found in production that are not present in the
real-email test corpus:
  - Out-of-range timezone offsets (e.g. +9900, +2400)
  - Missing Date header entirely

For each case the parser should fall back to the Received header date.
"""

from freezegun import freeze_time
import io
import pytest
from dateutil import parser as date_parser

from mlh_parser.parser import parse_and_process_email

from .helpers import list_files_with_extension, map_to_file_extensions


directory = "./synthetic/"
email_files = list_files_with_extension(directory, ".eml")

date_test_files = [
    map_to_file_extensions(email_f, [".date.pytest"]) for email_f in email_files
]


@freeze_time("2025-12-21")
@pytest.mark.parametrize("email_file, date_file", date_test_files)
def test_synthetic_date_fallback(email_file, date_file) -> None:
    mail_bytes = io.open(email_file, mode="rb").read()
    expected_date = date_parser.parse(
        io.open(date_file, mode="r", encoding="utf-8").read().split("\n")[0].strip()
    )

    result = parse_and_process_email(mail_bytes)

    assert result["date"] == expected_date, (
        f"Expected {expected_date}, got {result['date']}"
    )
