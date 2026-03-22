import logging
import email.generator
import email.header
import email.parser
import email.policy
import email.quoprimime
import email.utils
from email.message import EmailMessage
from typing import (
    Dict,
)


logger = logging.getLogger("email_reader")


def decode_mail(email_raw) -> str:
    # TODO: test
    # policy = email.policy.smtp
    policy = email.policy.default
    msg = email.parser.BytesParser(policy=policy).parsebytes(email_raw)
    return msg


def get_headers(msg: EmailMessage) -> Dict[str, str | list[str]]:
    headers = {}
    for key, item in msg.items():
        key = key.lower()
        if key in headers:
            existing = headers.get(key)
            # if field if list, append new value
            if isinstance(existing, list):
                headers[key].append(item)
            else:
                headers[key] = [existing, item]
        else:
            headers[key] = item
    return headers


def get_body(msg: EmailMessage) -> str:
    try:
        charset = msg.get_content_charset()

        body = msg.get_payload(decode=True)
        text = ""
        if body is not None:
            text = body.decode(charset or "utf-8", errors="replace")
        else:
            return ""
        return text
    except Exception as e:
        logger.error("failed loading body", e)
        return ""
