import pytest
import io
from mlh_parser.email_reader import decode_mail, get_body, get_headers
from .helpers import list_files_with_extension, map_to_file_extensions


directory = "./complete_cases/"
email_files = list_files_with_extension(directory, ".eml")

body_mail_files = [
    map_to_file_extensions(email_f, [".body.pytest"]) for email_f in email_files
]


@pytest.mark.parametrize("email_file, body_file", body_mail_files)
def test_body_parser(email_file, body_file) -> None:
    mail_text = io.open(email_file, mode="rb").read()
    # Read expected body and normalize line endings (CRLF -> LF)
    body = io.open(body_file, mode="r", encoding="utf-8").read().replace("\r\n", "\n")

    mail = decode_mail(mail_text)
    body_from_email = get_body(mail)

    assert body_from_email == body

