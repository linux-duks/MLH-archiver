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


def _normalize_email_in_trailers(trailers):
    """Normalize (a) obfuscation to @ in trailer identifications."""
    result = []
    for t in trailers:
        ident = t.get("identification", "").replace("(a)", "@")
        result.append(
            {"attribution": t.get("attribution", ""), "identification": ident}
        )
    return result


@pytest.mark.parametrize("email_file, code_file, trailers_file", complete_mail_files)
def test_complete_parser(email_file, code_file, trailers_file, subtests) -> None:
    mail_text = io.open(email_file, mode="rb").read()
    expected_trailers = eval(io.open(trailers_file, mode="r", encoding="utf-8").read())

    output = parse_and_process_email(mail_text)

    # Compare trailers as lists (normalize (a) to @ for comparison)
    output_trailers_normalized = _normalize_email_in_trailers(output["trailers"])
    expected_trailers_normalized = _normalize_email_in_trailers(expected_trailers)

    trailers_file_name = trailers_file.split("/")[-1]
    with subtests.test(msg=f"Checking value for trailers {trailers_file_name}"):
        assert output_trailers_normalized == expected_trailers_normalized, (
            f"trailers should match for {email_file}"
        )

    # TODO: fix code reading https://github.com/linux-duks/MLH-archiver/issues/15
    # # Read expected body and normalize line endings (CRLF -> LF)
    # expected_code = [
    #     code.replace("\r\n", "\n")
    #     for code in eval(io.open(code_file, mode="r", encoding="utf-8").read())
    # ]
    # code_file_name = code_file.split("/")[-1]
    # with subtests.test(msg=f"Checking value for code {code_file_name}"):
    #     # Compare code as lists of strings (exact match)
    #     assert output["code"] == expected_code, f"code should match for {email_file}"
