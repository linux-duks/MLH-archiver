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


def _ctx_log(ctx: dict, level: str, message: str, *args):
    """Helper to log with context information."""
    file_name = ctx.get("file_name", "unknown") if ctx else "unknown"
    mailing_list = ctx.get("mailing_list", "unknown") if ctx else "unknown"
    prefix = f"[{mailing_list}/{file_name}]"
    getattr(logger, level)(f"{prefix} {message}", *args)


def decode_mail(email_raw, ctx: dict = None) -> EmailMessage:
    """
    Parse raw email bytes into an EmailMessage.

    Args:
        email_raw: Raw email bytes
        ctx: Context dict with file_name, mailing_list, errors

    Returns:
        EmailMessage object
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    # policy = email.policy.smtp
    policy = email.policy.default
    msg = email.parser.BytesParser(policy=policy).parsebytes(email_raw)
    return msg


def get_headers(msg: EmailMessage, ctx: dict = None) -> Dict[str, str | list[str]]:
    """
    Extract headers from email message.

    Args:
        msg: EmailMessage object
        ctx: Context dict with file_name, mailing_list, errors

    Returns:
        Dictionary of email headers
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    headers = {}
    try:
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
    except Exception as e:
        _ctx_log(ctx, "error", "Failed to extract headers: %s", e)
        ctx["errors"].append(f"get_headers: {e}")

    return headers


def get_body(msg: EmailMessage, ctx: dict = None) -> str:
    """
    Extract body from email message.

    Args:
        msg: EmailMessage object
        ctx: Context dict with file_name, mailing_list, errors

    Returns:
        Email body as string
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

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
        _ctx_log(ctx, "error", "Failed to extract body: %s", e)
        ctx["errors"].append(f"get_body: {e}")
        return ""
