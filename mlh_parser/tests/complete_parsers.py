import pytest
import io
from mlh_parser.parser import parse_and_process_email
from .helpers import list_files_with_extension, map_to_file_extensions


directory = "./complete_cases/"
email_files = list_files_with_extension(directory, ".eml")

complete_mail_files = [
    map_to_file_extensions(email_f, [".code.pytest", ".trailers.pytest"])
    for email_f in email_files
]


@pytest.mark.parametrize("email_file, code_file, trailers_file", complete_mail_files)
def test_complete_mails(email_file, code_file, trailers_file) -> None:
    mail_text = io.open(email_file, mode="rb").read()
    code = eval(io.open(code_file, mode="r", encoding="utf-8").read())
    trailers = eval(io.open(trailers_file, mode="r", encoding="utf-8").read())

    output = parse_and_process_email(mail_text)

    assert str(trailers) == output["trailers"], (
        f"trailers should match for {email_file}"
    )
    assert str(code) == output["code"], f"code should match for {email_file}"
