import re
import logging
from mlh_parser.email_reader import decode_mail, get_body, get_headers
from mlh_parser.constants import (
    KEYS_MASK,
    SIGNED_BLOCK,
)

logger = logging.getLogger(__name__)


# TODO: use to filter for wanted headers only
def filter_data(data: dict) -> dict:
    result_dict = {key: data.get(key, "") for key in KEYS_MASK}
    return result_dict


# extract_attributions adapted from duks
# https://archive.softwareheritage.org/swh:1:cnt:23277d72e0a7a6f76db8542b10ab36e02a1e6006;origin=https://github.com/linux-duks/DUKS;visit=swh:1:snp:7539517b28d48726b43e4ef69332f3168b251aa0;anchor=swh:1:rev:de4af688e88757f0d496c7a16e331845a40d3f1c;path=/scripts/grpc_script.py;lines=82
def extract_attributions(commit_message, ctx: dict = None) -> list[dict] | list[str]:
    """
    Parses a git commit message and extracts all personal attributions.

    Args:
        commit_message (str): The full git commit message.
        ctx: Context dict with file_name, mailing_list, errors

    Returns:
        list: A list of dictionaries, where each dictionary contains
              'type' (e.g., 'Signed-off-by'), 'name', and 'email' for each attribution found.
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    # This regex looks for lines starting with "Word-by:"
    # followed by a name (can contain various characters), and then an email in angle brackets.
    # It captures the "Word-by" type, the name, and the email separately.
    # The pattern is compiled with MULTILINE to match '^' at the start of each line,
    # and IGNORECASE to match "Signed-off-by", "signed-off-by", etc.

    attributions = []
    # Ignore everything below standard email signature marker
    body = commit_message.split("\n-- \n", 1)[0].strip() + "\n"
    # Fix some more common copypasta trailer wrapping
    # Fixes: abcd0123 (foo bar
    # baz quux)
    body = re.sub(
        r"^(\S+:\s+[\da-f]+\s+\([^)]+)\n([^\n]+\))", r"\1 \2", body, flags=re.M
    )
    # Signed-off-by: Long Name
    # <email.here@example.com>
    body = re.sub(r"^(\S+:\s+[^<]+)\n(<[^>]+>)$", r"\1 \2", body, flags=re.M)
    # Signed-off-by: Foo foo <foo@foo.com>
    # [for the thing that the thing is too long the thing that is
    # thing but thing]
    pattern = re.compile(
        r"^\s*(?P<type>[a-zA-Z\-]+-by):[ \t]*(?P<name>[^<\n]+?)[ \t]*<(?P<email>[^>\n]+)>",
        re.MULTILINE | re.IGNORECASE,
    )

    # Use finditer to get all non-overlapping matches
    for match in pattern.finditer(commit_message):
        attributions.append(
            {
                "attribution": match.group(
                    "type"
                ).strip(),  # Extract the attribution type (e.g., 'Signed-off-by')
                "identification": match.group("name").strip()
                + f""" <{
                    match.group("email").strip()
                }>""",  # Extract the name (e.g., 'Author Name')
            }
        )
    return attributions


def extract_patches(email_body, ctx: dict = None) -> list[str]:
    """
    Extracts patch content from email body.

    Args:
        email_body: Email body text
        ctx: Context dict with file_name, mailing_list, errors

    Returns:
        list: List of patch strings found in the email
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    patches = []
    # a few regex options that will match a few styles found in patches
    regexes = [
        r"(^---$[\s\S]*?^--\s*\n+^.*$)",
        r"(^---$[\s\S]*?^--[\s=]*$\n+^.*$)",
        r"(diff --git[\s\S]*?^--\s*\n+^.*$)",
        r"(^---$[\s\S]*?^--*[\S\s=]*$\n+^.*$)",
    ]
    for pattern in regexes:
        op = re.compile(
            pattern,
            re.MULTILINE | re.IGNORECASE,
        )

        # Use finditer to get all non-overlapping matches
        matches = op.finditer(email_body)
        for match in matches:
            value = match.group(0).strip()
            if value:
                patches.append(
                    value,
                )
        # if there is a match, return it.
        # Otherwise, try the next regex
        if patches:
            return patches

        # if no match was found, return empty list (for clarity)
    match = re.search(r"^diff", email_body, re.MULTILINE)
    return []


def parse_email_bytes_to_dict(email_raw: bytes, ctx: dict = None) -> dict:
    """
    Parses raw email bytes into a dictionary.

    Args:
        email_raw: Raw email bytes
        ctx: Context dict with file_name, mailing_list, errors

    Returns:
        dict: Parsed email data
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    msg = decode_mail(email_raw, ctx=ctx)

    data = get_headers(msg, ctx=ctx)
    data["raw_body"] = get_body(msg, ctx=ctx)

    # TODO: refactor
    data[SIGNED_BLOCK] = extract_attributions(data["raw_body"], ctx=ctx)

    try:
        data["code"] = extract_patches(data["raw_body"], ctx=ctx)
    except Exception as e:
        logger.error(
            "[%s/%s] Failed to extract patches: %s",
            ctx["mailing_list"],
            ctx["file_name"],
            e,
        )
        ctx["errors"].append(f"extract_patches: {e}")
        raise e

    # data = filter_data(data)

    # result_dict = {}
    #
    # for header, value in data.items():
    #     result_dict[header] = str(value)

    return data
