// TODO: NEEDS REWRITE

import datetime
import re
from email import utils
from dateutil import parser as date_parser
import datetime as dt
import functools
import logging

logger = logging.getLogger(__name__)


def process_date(email_as_dict):
    if not isinstance(email_as_dict["date"], list):
        email_as_dict["date"] = [email_as_dict["date"]]

    # fill client-date column, with all dates reported by the client
    email_as_dict["client-date"] = email_as_dict["date"]
    # the "date" column will be replaced by our best-effort
    #  to parse and correct the actual email date

    date_options = []
    for date in email_as_dict["date"]:
        if date is None:
            continue
        date = date.strip()

        res = StringDateFinder.search(date)
        date_value = parse_date_tentative(res)
        if date_value:
            date_options.append(date_value)

    # detect if found dates are valid
    safe_options = [date for date in date_options if not check_date_issues(date)]

    # if declared dates are not trustworthy, lets try to find dates from other headers
    if not safe_options:
        options = find_other_date_entries(email_as_dict)
        if options:
            logging.debug(
                f"Found {len(options)} other options when reading from other headers"
            )
            safe_options = options

    # if still no good options were found, letry try to fix "millennium dates"
    if not safe_options:
        millennium_dates = [date for date in date_options if is_date_too_old(date)]
        for date in millennium_dates:
            safe_options.append(fix_milenium_date(date))
        logging.debug(
            f"Found only millennium dates, and fixed {len(millennium_dates)} on them"
        )

    if safe_options:
        # take the oldest date
        safe_options.sort()
        email_as_dict["date"] = safe_options[0]
    else:
        email_as_dict["date"] = None

    return email_as_dict


## date parser functions


def last_effort_date_finder(date_text):
    date = None
    if "(" in date_text:
        date_text = date_text[: date_text.index("(")].strip()

    try:
        date = date_parser.parse(date_text)
    except Exception:
        try:
            date = date_parser.parse(date_text.replace(".", ":"))
        except Exception:
            try:
                date = date_parser.parse(
                    date_text[: len("Fri, 15 Jun 2012 16:52:52")].strip(),
                )
            except Exception:
                try:
                    date = date_parser.parse(
                        date_text[: len("Fri, 5 Jun 2012 16:52:52")].strip(),
                    )
                except Exception:
                    date = None
    return date


def _has_valid_utc_offset(date_value) -> bool:
    """Return False if the timezone offset is outside Python's valid ±24h range."""
    try:
        offset = date_value.utcoffset()
    except ValueError:
        return False
    if offset is None:
        return True
    return dt.timedelta(hours=-24) < offset < dt.timedelta(hours=24)


def parse_date_tentative(date):
    if not date:
        return None
    date_value = None
    try:
        date_value = date_parser.parse(date)
    except Exception as e:
        try:
            date_value = utils.parsedate_to_datetime(date)
        except Exception as ee:
            date_value = last_effort_date_finder(date)
            if not date_value:
                logging.debug("failed reading date with all parsers: %s | %s", e, ee)
                raise ValueError(f"Could not parse date {date!r}") from ee
            else:
                logging.debug(
                    "failed reading date with standard parsers, used fallback: %s | %s",
                    e,
                    ee,
                )

    if date_value is not None and not _has_valid_utc_offset(date_value):
        logging.warning(f"Discarding date with invalid UTC offset: {date!r}")
        return None
    return date_value


# returns true if there are issues with the found dates
@functools.lru_cache(maxsize=128)
def check_date_issues(date_obj) -> bool:
    return not date_obj or is_date_too_old(date_obj) or is_date_future(date_obj)


@functools.lru_cache(maxsize=128)
def is_date_too_old(date_obj) -> bool:
    return date_obj.year < 1900


@functools.lru_cache(maxsize=128)
def fix_milenium_date(date_obj) -> datetime.datetime:
    # dates from 1999 may be represented by 99
    # and the real millennium bug exists here:
    # 100 represents year 2000

    # for this to be considered, 1900 + the obj year should not be more than the current year
    if date_obj.year < (get_forgiving_future_date().year - 1900):
        date_obj = date_obj.replace(year=date_obj.year + 1900)
    else:
        logger.debug("Date is not a `millennium date`")
    return date_obj


@functools.lru_cache(maxsize=128)
def is_date_future(date_obj) -> bool:
    future = get_forgiving_future_date()
    # Make comparison timezone-safe: if date_obj is aware, make future aware too
    if date_obj.tzinfo is not None:
        # Use UTC for the future date comparison
        future = dt.datetime.now(dt.timezone.utc) + dt.timedelta(days=3)
    return date_obj > future


# singleton date. No need to reload it every time
NEAR_NOW = None


def get_forgiving_future_date():
    global NEAR_NOW
    if NEAR_NOW is None:
        NEAR_NOW = datetime.datetime.now() + datetime.timedelta(days=3)
    return NEAR_NOW


# headers where dates can be found, besides "Date"
fallback_date_headers = ["Received", "X-Received"]


# find_other_date_entries looks for dates in other columns, and returns them parsed, if found
def find_other_date_entries(email_as_dict):
    value_list = []
    logging.debug(f"emails header list: {email_as_dict.keys()}")
    for col_opt in fallback_date_headers:
        values = email_as_dict.get(col_opt.lower())

        if values:
            if not isinstance(values, list):
                values = [values]

            for val in values:
                res = StringDateFinder.search(str(val).strip())
                if res:
                    parsed_res = parse_date_tentative(res)
                    if parsed_res:
                        value_list.append(parsed_res)
    return value_list


# StringDateFinder singleton instance
find_date_in_string = None


# StringDateFinder singleton date finder class
class StringDateFinder:
    compiled = False
    datepattern = None

    def __init__(self):
        rfc2822 = r"(?:(Sun|Mon|Tue|Wed|Thu|Fri|Sat),\s+)?(0[1-9]|[1-2]?[0-9]|3[01])\s+(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+([0-1][0-9]{3}|[0-1][0-9]{2}|19[0-9]{2}|[2-9][0-9]{3})\s+(2[0-3]|[0-1][0-9]):([0-5][0-9])(?::(60|[0-5][0-9]))?\s+([-\+][0-9]{2}[0-5][0-9]|(?:UT|GMT|(?:E|C|M|P)(?:ST|DT)|[A-IK-Z]))(\s+|\(([^\(\)]+|\\\(|\\\))*\))*"
        pat1123 = r"\w{3}, \d{2} \w{3} \d{4} \d{2}:\d{2}:\d{2} \w{3}"
        pat1036 = r"\w+?, \d{2}-\w{3}-\d{2} \d{2}:\d{2}:\d{2} \w{3}"
        patc = r"\w{3} \w{3} \d+? \d{2}:\d{2}:\d{2} \d{4}"

        self.datepattern = re.compile(
            "(?:%s)|(?:%s)|(?:%s)|(?:%s)"
            % (
                rfc2822,
                pat1123,
                pat1036,
                patc,
            )
        )
        self.compiled = True

    def get():
        global find_date_in_string
        if find_date_in_string is None:
            find_date_in_string = StringDateFinder()
        return find_date_in_string

    def search(text):
        self = StringDateFinder.get()

        res = self.datepattern.search(text)
        if res:
            res = res.group()

        return res
