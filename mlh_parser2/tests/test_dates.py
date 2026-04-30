from freezegun import freeze_time
import io
import pytest
from dateutil import parser as date_parser

from mlh_parser.email_reader import decode_mail, get_headers
from mlh_parser.date_parser import process_date, fix_milenium_date

from .helpers import (
    list_files_with_extension,
    map_to_file_extensions,
    resolve_test_file_path,
)

directory = "./date_cases/"
email_files = list_files_with_extension(directory, ".eml")

real_mail_files = [
    map_to_file_extensions(email_f, [".date.pytest"]) for email_f in email_files
]


@freeze_time("2025-12-21")
@pytest.mark.parametrize("email_file, date_file", real_mail_files)
def test_correct_email(email_file, date_file) -> None:
    mail_bytes = io.open(email_file, mode="rb").read()
    expected_date = date_parser.parse(
        # only take the first line of this file, as the rest is used for comments
        io.open(date_file, mode="r", encoding="utf-8").read().split("\n")[0].strip()
    )

    msg = decode_mail(mail_bytes)
    charset = msg.get_content_charset()
    print("charset", charset)

    headers = get_headers(msg)
    response = process_date(headers)

    print("date", response["date"])
    assert response["date"] == expected_date


multi_date_email_case = "todo-find-real-case"
multi_date_email_case_files = [
    (
        resolve_test_file_path(directory, multi_date_email_case + ".eml"),
        resolve_test_file_path(directory, multi_date_email_case + ".date.pytest"),
        resolve_test_file_path(
            directory, multi_date_email_case + ".client-date.pytest"
        ),
    ),
]


@freeze_time("2025-12-21")
@pytest.mark.parametrize(
    "email_file, date_file, client_date_file", multi_date_email_case_files
)
def test_multiple_dates_email(email_file, date_file, client_date_file) -> None:
    mail_bytes = io.open(email_file, mode="rb").read()
    expected_date = date_parser.parse(
        # only take the first line of this file, as the rest is used for comments
        io.open(date_file, mode="r", encoding="utf-8").read().split("\n")[0].strip()
    )
    client_dates = [
        d.strip()
        for d in
        # one "client-date" per line
        io.open(client_date_file, mode="r", encoding="utf-8").read().split("\n")
        if d.strip()  # filter out empty lines
    ]

    msg = decode_mail(mail_bytes)
    charset = msg.get_content_charset()
    print("charset", charset)

    headers = get_headers(msg)
    response = process_date(headers)

    print("date", response["date"])
    assert response["date"] == expected_date
    assert response["client-date"] == client_dates


millennium_dates = [
    ("Mon, 3 Jan 78 18:27:37", "Mon, 3 Jan 1978 18:27:37"),
    ("Mon, 3 Jan 99 18:27:37", "Mon, 3 Jan 99 18:27:37"),
    ("Mon, 3 Jan 100 18:27:37", "Mon, 3 Jan 2000 18:27:37"),
    ("Mon, 3 Jan 0100 18:27:37", "Mon, 3 Jan 2000 18:27:37"),
    ("Mon, 3 Jan 101 18:27:37", "Mon, 3 Jan 2001 18:27:37"),
    ("Mon, 3 Jan 0120 18:27:37", "Mon, 3 Jan 2020 18:27:37"),
]


# test focused only on the "millennium_dates" correction developped
@freeze_time("2025-12-21")
@pytest.mark.parametrize("found_date, expected_date", millennium_dates)
def test_millennium_dates(found_date, expected_date):
    expected_date = date_parser.parse(expected_date)
    found_date = date_parser.parse(found_date)
    fixed_date = fix_milenium_date(found_date)
    assert expected_date == fixed_date
