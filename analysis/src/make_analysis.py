import polars as pl
from datetime import datetime, timedelta

import numpy as np
import seaborn as sns
import matplotlib.pyplot as plt
import re

sns.set_style("whitegrid")

# in NNTP
# LISTS_OF_INTEREST = [
#     "org.freedesktop.lists.amd-gfx",
#     "org.freedesktop.lists.intel-gfx",
#     "org.kernel.vger.linux-iio",
#     "org.kernel.vger.rust-for-linux",
# ]
# in PublicInbox
LISTS_OF_INTEREST = [
    "amd-gfx",
    "intel-gfx",
    "linux-iio",
    "rust-for-linux",
]


df = pl.read_parquet("/input/list=" + LISTS_OF_INTEREST[0] + "/")
df = df.with_columns(pl.lit(LISTS_OF_INTEREST[0]).alias("list"))

for i in range(1, len(LISTS_OF_INTEREST)):
    new_list_df = pl.read_parquet(
        "/input/list=" + LISTS_OF_INTEREST[i] + "/"
    )
    new_list_df = new_list_df.with_columns(pl.lit(LISTS_OF_INTEREST[i]).alias("list"))
    df.extend(new_list_df)

df = df.filter(pl.col("date") > datetime(2020, 1, 1))
df = df.sort("date")

WINDOW_SIZE = 60
DATESAMPLINGINTERVAL = 5


def retrieve_reviewers_and_testers(sorted_df):
    FIRSTCOMMITDATE = sorted_df[0]["date"][0] + timedelta(days=WINDOW_SIZE)
    LASTCOMMITDATE = sorted_df[-1]["date"][0]

    results = {}

    for mail_list in LISTS_OF_INTEREST:
        results[mail_list] = {
            "running_reviewed": 0,
            "running_tested": 0,
            "reviewed_points": [],
            "tested_points": [],
            "any_points": [],
        }

    window_begin = 0
    window_end = -1

    thisDate = FIRSTCOMMITDATE
    all_dates = []
    while thisDate < LASTCOMMITDATE:
        maxDate = thisDate + timedelta(days=1)
        minDate = thisDate + timedelta(days=-WINDOW_SIZE)

        # First, update the datetime window. Starting with the last commit of the window
        while (
            window_end < len(sorted_df) - 1
            and maxDate >= sorted_df[window_end + 1]["date"][0]
        ):
            window_end += 1

            this_email = sorted_df[window_end]

            this_list = this_email["list"][0]
            trailers = this_email["trailers"][0]

            if len(trailers) == 0:
                continue

            for signature in trailers:
                attr = signature["attribution"]

                if re.match(r"reviewed-by", attr, re.IGNORECASE):
                    results[this_list]["running_reviewed"] += 1
                elif re.match(r"tested-by", attr, re.IGNORECASE):
                    results[this_list]["running_tested"] += 1

        # Update window beginning
        while (
            window_begin < len(sorted_df) - 1
            and minDate > sorted_df[window_begin]["date"][0]
        ):
            window_begin += 1

            this_email = sorted_df[window_begin]
            this_list = this_email["list"][0]
            trailers = this_email["trailers"][0]

            if len(trailers) == 0:
                continue

            for signature in trailers:
                attr = signature["attribution"]

                if re.match(r"reviewed-by", attr, re.IGNORECASE):
                    results[this_list]["running_reviewed"] -= 1
                elif re.match(r"tested-by", attr, re.IGNORECASE):
                    results[this_list]["running_tested"] -= 1

        for mailing_list in results:
            results[mailing_list]["reviewed_points"].append(
                results[mailing_list]["running_reviewed"]
            )
            results[mailing_list]["tested_points"].append(
                results[mailing_list]["running_tested"]
            )
            results[mailing_list]["any_points"].append(
                results[mailing_list]["running_reviewed"]
                + results[mailing_list]["running_tested"]
            )
        all_dates.append(thisDate)
        thisDate = thisDate + timedelta(days=DATESAMPLINGINTERVAL)

    for mailing_list in results:
        results[mailing_list] = results[mailing_list]["any_points"]
    return all_dates, results


dates, list_results = retrieve_reviewers_and_testers(df)

# Plot each line
for lista in LISTS_OF_INTEREST:
    plt.plot(dates, list_results[lista], label=lista)

# Add labels and title
plt.xlabel("Patch Date")
plt.ylabel("Contributors")
plt.title("Auxiliary Contributors")

# Add a legend to distinguish the lines
plt.legend()

# Display the plot
plt.show
plt.savefig("/app/results/auxContribs.svg")


#### Date analysis

lazy_df = pl.scan_parquet("/input/")
email_dates = lazy_df.describe()["date"]
min_date = email_dates[4]
quartile_dates = email_dates[5]

with open("/app/results/date_stats.txt", "w") as date_file:
    date_file.write(min_date + "\n" + quartile_dates)
del lazy_df

### List Sizes

import os

list_sizes = []
biggest_list = (-1, "none")
smallest_list = (999999999999999, "none")

for subdir_name in os.listdir("/input/"):
    list_name = subdir_name.split("=")[1]

    sub_df = pl.scan_parquet("/input/" + subdir_name + "/")
    list_size = int(sub_df.describe()["from"][0])

    list_sizes.append(list_size)

    if list_size > biggest_list[0]:
        biggest_list = (list_size, list_name)

    if list_size < smallest_list[0]:
        smallest_list = (list_size, list_name)

with open("/app/results/list_sizes.txt", "w") as list_data_file:
    list_data_file.write("Min:" + str(smallest_list))
    list_data_file.write("\nQ1:" + str(np.percentile(list_sizes, 25)))
    list_data_file.write("\nQ2:" + str(np.percentile(list_sizes, 50)))
    list_data_file.write("\nQ3:" + str(np.percentile(list_sizes, 75)))
    list_data_file.write("\nMax:" + str(biggest_list))

