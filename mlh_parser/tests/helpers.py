import os
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
