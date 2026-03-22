import logging
import email.generator
import email.header
import email.parser
import email.policy
import email.quoprimime
import email.utils
import re
from email.message import EmailMessage
from typing import (
    Dict,
)


logger = logging.getLogger("email_reader")

# Pattern to match common email header lines at the start of body content
# These are often added by git send-email or mailing list software
HEADER_LINE_PATTERN = re.compile(
    r'^(From|To|Cc|Subject|Date|Message-ID|In-Reply-To|References|User-Agent|X-Mailer):[ \t]*.*$',
    re.IGNORECASE | re.MULTILINE
)


def _ctx_log(ctx: dict, level: str, message: str, *args):
    """Helper to log with context information."""
    file_name = ctx.get("file_name", "unknown") if ctx else "unknown"
    mailing_list = ctx.get("mailing_list", "unknown") if ctx else "unknown"
    prefix = f"[{mailing_list}/{file_name}]"
    getattr(logger, level)(f"{prefix} {message}", *args)


def _clean_body_leading_headers(body: str) -> str:
    """
    Remove leading header-like lines from email body.
    
    Mailing list emails often have header-like lines at the start of the body
    (e.g., "From: Author <email>" from git send-email) that should be stripped.
    
    Args:
        body: Raw email body text
        
    Returns:
        Body with leading header lines removed
    """
    if not body:
        return body
    
    lines = body.split('\n')
    start_idx = 0
    
    # Skip leading header-like lines
    for i, line in enumerate(lines):
        stripped = line.strip()
        if not stripped:
            # Empty line - stop here, headers should be contiguous
            start_idx = i + 1
            break
        if HEADER_LINE_PATTERN.match(stripped):
            start_idx = i + 1
        else:
            # Non-header line found, stop
            break
    
    # Also skip any blank lines after the headers
    while start_idx < len(lines) and not lines[start_idx].strip():
        start_idx += 1
    
    return '\n'.join(lines[start_idx:]) if start_idx > 0 else body


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

    def __get_body():
        # For multipart emails, walk through parts and extract text content
        if msg.is_multipart():
            body_parts = []
            for part in msg.walk():
                content_type = part.get_content_type()
                content_disposition = str(part.get_content_disposition() or "")

                # Skip attachments and non-text parts
                if content_disposition.startswith("attachment"):
                    continue
                if content_type not in ("text/plain", "text/html"):
                    continue

                try:
                    payload = part.get_payload(decode=True)
                    if payload is None:
                        continue

                    charset = part.get_content_charset()

                    # Validate charset - common malformed values include 'y', 'yes', etc.
                    if charset:
                        try:
                            import codecs

                            codecs.lookup(charset)
                        except LookupError, ValueError:
                            _ctx_log(
                                ctx,
                                "warning",
                                "Invalid charset '%s', using utf-8",
                                charset,
                            )
                            charset = "utf-8"

                    text = payload.decode(charset or "utf-8", errors="replace")
                    body_parts.append(text)
                except Exception as part_error:
                    _ctx_log(ctx, "warning", "Failed to decode part: %s", part_error)
                    ctx["errors"].append(f"get_body part: {part_error}")
                    continue

            return "\n".join(body_parts)

        # For single-part emails
        charset = msg.get_content_charset()
        body = msg.get_payload(decode=True)

        if body is None:
            return ""

        # Validate charset
        if charset:
            try:
                import codecs

                codecs.lookup(charset)
            except LookupError, ValueError:
                _ctx_log(ctx, "warning", "Invalid charset '%s', using utf-8", charset)
                charset = "utf-8"

        return body.decode(charset or "utf-8", errors="replace")

    try:
        body = __get_body().replace("\r\n", "\n")
        # Clean up leading header-like lines from body content
        return _clean_body_leading_headers(body)
    except Exception as e:
        _ctx_log(ctx, "error", "Failed to extract body: %s", e)
        ctx["errors"].append(f"get_body: {e}")
        return ""
