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

# Pattern to match valid email addresses with optional name
# Matches: "Name <email@domain>" or "email@domain"
EMAIL_PATTERN = re.compile(
    r'^[\s]*([^<]*?)?\s*<?([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?\s*$'
)

# Pattern to match obfuscated email with (a)
EMAIL_OBFUSCATED_A_PATTERN = re.compile(
    r'^[\s]*([^<]*?)?\s*<?([a-zA-Z0-9._%+-]+)\s*\(a\)\s*([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?\s*$'
)

# Pattern to match obfuscated email with " at "
EMAIL_OBFUSCATED_AT_PATTERN = re.compile(
    r'^[\s]*([^<]*?)?\s*<?([a-zA-Z0-9._%+-]+)\s+at\s+([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?\s*$',
    re.IGNORECASE
)


def _ctx_log(ctx: dict, level: str, message: str, *args):
    """Helper to log with context information."""
    file_name = ctx.get("file_name", "unknown") if ctx else "unknown"
    mailing_list = ctx.get("mailing_list", "unknown") if ctx else "unknown"
    prefix = f"[{mailing_list}/{file_name}]"
    getattr(logger, level)(f"{prefix} {message}", *args)


def _is_valid_email_address(value: str) -> bool:
    """
    Check if a header value contains a valid email address.

    Valid formats:
    - "Name <email@domain.com>"
    - "email@domain.com"

    Invalid formats:
    - "user at domain.com" (no @ symbol)
    - "user @ domain" (spaces around @)

    Args:
        value: Header value string

    Returns:
        True if valid email address format found
    """
    return EMAIL_PATTERN.match(value) is not None


def _score_email_address(value: str) -> tuple:
    """
    Score an email address by quality.
    
    Returns a tuple (has_name, has_valid_email, obfuscation_type) for sorting.
    Higher scores are better.
    
    Scoring:
    - (True, True, None) = Best: "Name <email@domain>"
    - (False, True, None) = Good: "email@domain"
    - (True, False, '(a)') = Medium: "Name <email(a)domain>"
    - (False, False, '(a)') = Low: "email(a)domain"
    - (True, False, ' at ') = Low: "Name <email at domain>"
    - (False, False, ' at ') = Lowest: "email at domain"
    
    Args:
        value: Email address string
        
    Returns:
        Tuple (has_name: bool, has_valid_email: bool, obfuscation: str|None)
    """
    # Check for valid email with @
    match = EMAIL_PATTERN.match(value)
    if match:
        name = match.group(1).strip() if match.group(1) else ""
        return (bool(name), True, None)
    
    # Check for (a) obfuscation
    match = EMAIL_OBFUSCATED_A_PATTERN.match(value)
    if match:
        name = match.group(1).strip() if match.group(1) else ""
        return (bool(name), False, '(a)')
    
    # Check for " at " obfuscation
    match = EMAIL_OBFUSCATED_AT_PATTERN.match(value)
    if match:
        name = match.group(1).strip() if match.group(1) else ""
        return (bool(name), False, ' at ')
    
    # No valid pattern found
    return (False, False, None)


def _normalize_email(value: str) -> str:
    """
    Normalize an email address by converting obfuscation to @.
    
    Converts:
    - "user(a)domain.com" -> "user@domain.com"
    - "user at domain.com" -> "user@domain.com"
    
    Args:
        value: Email address string (possibly obfuscated)
        
    Returns:
        Normalized email address
    """
    # Check for (a) obfuscation
    match = EMAIL_OBFUSCATED_A_PATTERN.match(value)
    if match:
        name = match.group(1).strip() if match.group(1) else ""
        email = f"{match.group(2)}@{match.group(3)}"
        if name:
            return f"{name} <{email}>"
        return email
    
    # Check for " at " obfuscation
    match = EMAIL_OBFUSCATED_AT_PATTERN.match(value)
    if match:
        name = match.group(1).strip() if match.group(1) else ""
        email = f"{match.group(2)}@{match.group(3)}"
        if name:
            return f"{name} <{email}>"
        return email
    
    # Already valid or unknown format, return as-is
    return value


def _select_best_from_header(values: list | str) -> str:
    """
    Select the best 'From' header value from multiple candidates.
    
    Scoring priority (best to worst):
    1. Complete identity with valid email: "Name <email@domain>"
    2. Email only with valid format: "email@domain"
    3. Complete identity with (a) obfuscation: "Name <email(a)domain>"
    4. Email only with (a) obfuscation: "email(a)domain"
    5. Complete identity with " at " obfuscation: "Name <email at domain>"
    6. Email only with " at " obfuscation: "email at domain"
    
    Args:
        values: Single string or list of header values
        
    Returns:
        Best email header value, normalized
    """
    if isinstance(values, str):
        return _normalize_email(values)
    
    if not values:
        return ""
    
    # Score all values and sort by score (descending)
    scored = [(_score_email_address(v), v) for v in values]
    scored.sort(key=lambda x: x[0], reverse=True)
    
    # Return the best one, normalized
    best_value = scored[0][1]
    return _normalize_email(best_value)


def _clean_body_leading_headers(body: str, ctx: dict = None) -> str:
    """
    Remove leading header-like lines from email body.
    
    Mailing list emails often have header-like lines at the start of the body
    (e.g., "From: Author <email>" from git send-email) that should be stripped.
    
    If a valid 'From:' line is found in the body and the context has no valid
    'from' header, it will be stored in ctx['from'] for later use.

    Args:
        body: Raw email body text
        ctx: Context dict (optional, used to store extracted 'from' header)

    Returns:
        Body with leading header lines removed
    """
    if not body:
        return body

    lines = body.split('\n')
    start_idx = 0
    extracted_from = None

    # Skip leading header-like lines
    for i, line in enumerate(lines):
        stripped = line.strip()
        if not stripped:
            # Empty line - stop here, headers should be contiguous
            start_idx = i + 1
            break
        match = HEADER_LINE_PATTERN.match(stripped)
        if match:
            # Check if this is a valid From: line with email address
            if match.group(1).lower() == 'from':
                # Extract the value after "From:"
                from_value = stripped[5:].strip()  # Skip "From:"
                if _is_valid_email_address(from_value):
                    extracted_from = from_value
            start_idx = i + 1
        else:
            # Non-header line found, stop
            break

    # Also skip any blank lines after the headers
    while start_idx < len(lines) and not lines[start_idx].strip():
        start_idx += 1

    # Store extracted 'from' in context for get_headers to use
    if extracted_from and ctx is not None:
        ctx['extracted_from'] = extracted_from

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


def get_headers(msg: EmailMessage, ctx: dict = None, raw_email: bytes = None) -> Dict[str, str | list[str]]:
    """
    Extract headers from email message.

    Uses raw header access to avoid parsing errors with malformed addresses.
    Unfolds folded headers (RFC 5322) by replacing CRLF+whitespace with single space.
    
    For the 'from' header, collects all candidates (from headers and body),
    scores them by quality, and selects the best one.

    Args:
        msg: EmailMessage object
        ctx: Context dict with file_name, mailing_list, errors
        raw_email: Raw email bytes (used to extract 'from' from body if header is invalid)

    Returns:
        Dictionary of email headers with unfolded values
    """
    if ctx is None:
        ctx = {"file_name": "unknown", "mailing_list": "unknown", "errors": []}

    headers = {}
    from_candidates = []
    
    try:
        # Use raw headers to avoid parsing errors with malformed addresses
        for key, value in msg._headers:
            key = key.lower()
            # Unfold header: replace CRLF followed by whitespace with single space
            unfolded_value = ' '.join(value.split())
            
            if key == 'from':
                from_candidates.append(unfolded_value)
            elif key in headers:
                existing = headers.get(key)
                # if field if list, append new value
                if isinstance(existing, list):
                    existing.append(unfolded_value)
                else:
                    headers[key] = [existing, unfolded_value]
            else:
                headers[key] = unfolded_value
        
        # Extract additional From candidates from body
        if raw_email:
            body_from_candidates = _extract_all_from_from_body(raw_email)
            from_candidates.extend(body_from_candidates)
        
        # Select the best From header
        if from_candidates:
            headers['from'] = _select_best_from_header(from_candidates)
            
    except Exception as e:
        _ctx_log(ctx, "error", "Failed to extract headers: %s", e)
        ctx["errors"].append(f"get_headers: {e}")

    return headers


def _extract_all_from_from_body(raw_email: bytes) -> list:
    """
    Extract all 'From:' email addresses from the email body.
    
    This is used to find valid From headers that may appear in the body
    (common in git send-email patches).
    
    Handles common email obfuscation like "(a)" for "@" and " at " for "@".
    
    Args:
        raw_email: Raw email bytes
        
    Returns:
        List of From header values found in body
    """
    candidates = []
    try:
        # Decode email to text
        email_text = raw_email.decode('utf-8', errors='replace')
        
        # Look for pattern "From: Name <email@domain>" or "From: Name <email(a)domain>"
        # or "From: email at domain"
        from_patterns = [
            # Standard @ pattern
            re.compile(
                r'^From:\s*([^<\n]*?)?\s*<([a-zA-Z0-9._%+-]+)@([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>',
                re.MULTILINE | re.IGNORECASE
            ),
            # (a) obfuscation pattern
            re.compile(
                r'^From:\s*([^<\n]*?)?\s*<([a-zA-Z0-9._%+-]+)\s*\(a\)\s*([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>',
                re.MULTILINE | re.IGNORECASE
            ),
            # " at " obfuscation pattern
            re.compile(
                r'^From:\s*([^<\n]*?)?\s*<?([a-zA-Z0-9._%+-]+)\s+at\s+([a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>?',
                re.MULTILINE | re.IGNORECASE
            ),
        ]
        
        for pattern in from_patterns:
            for match in pattern.finditer(email_text):
                name = match.group(1).strip() if match.group(1) else ""
                # Reconstruct the full From value
                if len(match.groups()) >= 3:
                    email = f"{match.group(2)}@{match.group(3)}"
                    if name:
                        candidates.append(f"{name} <{email}>")
                    else:
                        candidates.append(email)
            
    except Exception:
        pass
    
    return candidates


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
