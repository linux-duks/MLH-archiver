import os
import io
from pathlib import Path


# helper functions
def list_files_with_extension(directory_path, extension):
    if not extension.startswith("."):
        extension = "." + extension  # Ensure the extension starts with a dot

    relpath = Path(__file__).parent.resolve()
    directory_path = relpath.joinpath(directory_path)
    files_with_extension = []
    for filename in os.listdir(directory_path):
        full_filename = os.path.join(directory_path, filename)
        if filename.endswith(extension) and os.path.isfile(full_filename):
            files_with_extension.append(full_filename)
    files_with_extension.sort()
    return files_with_extension


# return the original file and alternative extensions
def map_to_file_extensions(email_file_name, extensions):
    return (email_file_name,) + tuple(
        [
            email_file_name[:-4] + ext
            if email_file_name.endswith(".eml")
            else email_file_name + ext
            for ext in extensions
        ]
    )


def resolve_test_file_path(directory: str, filename: str) -> str:
    """Resolve a test file path to an absolute path.

    Args:
        directory: Relative path to the test directory (e.g., "./date_cases/")
        filename: Name of the file (e.g., "todo-find-real-case.eml")

    Returns:
        Absolute path to the file
    """
    relpath = Path(__file__).parent.resolve()
    directory_path = relpath.joinpath(directory)
    return str(directory_path / filename)


def parse_headers_file(headers_file: str) -> dict:
    """
    Parse a .headers.pytest file into a dictionary.

    The file format is raw email headers, ending at the first empty line
    or MIME boundary. Only the header section is parsed.

    Handles RFC 5322 header folding (continuation lines starting with
    space or tab are joined to the previous header).

    Returns a dict where values are either strings or lists of strings.
    """
    headers = {}
    current_header = None
    current_value = None

    with io.open(headers_file, mode="r", encoding="utf-8") as f:
        for line in f:
            line = line.replace("\r\n", "\n").rstrip("\n")

            # Stop at empty line or MIME boundary (end of headers)
            if not line.strip() or line.startswith("--"):
                break

            # Check if this is a continuation line (starts with space/tab)
            if line and line[0] in " \t":
                if current_header is not None:
                    # Append to previous header value
                    current_value += " " + line.strip()
                continue

            # Save previous header if exists
            if current_header is not None:
                if current_header in headers:
                    existing = headers[current_header]
                    if isinstance(existing, list):
                        existing.append(current_value)
                    else:
                        headers[current_header] = [existing, current_value]
                else:
                    headers[current_header] = current_value

            # Parse new header
            if ":" not in line:
                current_header = None
                current_value = None
                continue

            key, value = line.split(":", 1)
            current_header = key.strip().lower()
            current_value = value.strip()

    # Save last header
    if current_header is not None:
        if current_header in headers:
            existing = headers[current_header]
            if isinstance(existing, list):
                existing.append(current_value)
            else:
                headers[current_header] = [existing, current_value]
        else:
            headers[current_header] = current_value

    return headers
