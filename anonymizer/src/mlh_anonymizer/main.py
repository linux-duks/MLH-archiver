from multiprocessing import Pool
import os
import polars as pl
import hashlib
import logging
import itertools

from anonymizer.constants import (
    N_PROC,
    LISTS_TO_PARSE,
    ANONYMIZE_COLUMNS,
    ANONYMIZE_MAP,
    SPLIT_DATASET_COLUMNS,
)


DEBUG = os.getenv("DEBUG", "false")
level = logging.INFO
if DEBUG != "false":
    level = logging.DEBUG

logging.basicConfig(
    level=level,
    format="[%(asctime)s] {%(pathname)s:%(lineno)d} %(levelname)s - %(message)s",
    datefmt="%H:%M:%S",
)

logger = logging.getLogger(__name__)


# TODO: move to config location
INPUT_DIR_PATH = os.environ["INPUT_DIR"]
OUTPUT_DIR_PATH = os.environ["OUTPUT_DIR"]


def parse_mail_at(mailing_list):
    """
    Parses the emails from a single specified list,
    to be found in INPUT_DIR_PATH/mailing_list
    """

    input_path = INPUT_DIR_PATH + "/" + mailing_list

    def read_dataset():
        df = pl.DataFrame()
        try:
            df = pl.read_parquet(input_path)
            if df.limit(1).is_empty():
                return None
            return df

        except Exception as e:
            logger.error(f"Failed to read dataset from {input_path} error:", e)
            return None

    try:
        # all operations will be done on a list of dataframes
        # first, create the split datasets generator
        def dataset_generator() -> pl.DataFrame:
            for split_column in SPLIT_DATASET_COLUMNS:
                # use the base dataset every time
                df = read_dataset()
                if df is not None:
                    yield (
                        f"__id_map_{split_column}",
                        read_dataset().select(
                            pl.col(split_column).alias(f"__original_{split_column}"),
                            pl.col(split_column),
                        ),
                    )

        # run the main dataset first
        process_dataframe(read_dataset(), "__main_dataset", input_path, mailing_list)

        # run the first dataset before going into the generator
        for dataset_name, df in itertools.chain(dataset_generator()):
            process_dataframe(df, dataset_name, input_path, mailing_list)

    except Exception as e:
        raise (e)


def process_dataframe(df, dataset_name, input_path, mailing_list):
    if df is None:
        logger.warn(f"Dataset '{dataset_name}'.'{input_path}' did nor produce data")
        return None
    df_columns = df.collect_schema().names()
    for col in ANONYMIZE_COLUMNS:
        if col not in df_columns:
            logger.warn(f"Column {col} not available in dataset {dataset_name}")
            continue
        logger.info(f"Running '{col}'.'{dataset_name}'.'{input_path}'")
        df = df.with_columns(
            pl.col(col)
            .map_elements(lambda x: anonymizer(x), return_dtype=pl.self_dtype())
            .alias(col),
        )

    for col in ANONYMIZE_MAP:
        col_parts = col.split(".")
        if col not in df_columns:
            logger.warn(f"Column {col} not available in dataset {dataset_name}")
            continue
        logger.info(f"Running '{col}'.'{dataset_name}'.'{input_path}'")
        logger.info(
            f"Running map {col}. Will write '{col_parts[0]}' with '{col_parts[1]}' anonymized"
        )
        df = df.with_columns(
            pl.col(col_parts[0])
            .map_elements(
                lambda x: anonymize_map(x, col_parts[1]),
                return_dtype=pl.self_dtype(),
            )
            .alias(col_parts[0]),
        )

    output_path = OUTPUT_DIR_PATH + f"/{dataset_name}/" + mailing_list

    logger.info(f"Writing {output_path}")

    os.makedirs(output_path, exist_ok=True)
    df.write_parquet(
        output_path + "/data.parquet",
        compression="zstd",
        row_group_size=1024**2,  # double the default
        data_page_size=(1024 * 2) ** 2,
        compression_level=22,  # maximum compression for Zenodo
    )
    del df


def generate_sha1_hash(input_string):
    encoded_string = input_string.encode("utf-8")
    # Create an SHA-1 hash object
    sha1_hash_object = hashlib.sha1()
    sha1_hash_object.update(encoded_string)
    # Get the hexadecimal representation of the digest
    hex_digest = sha1_hash_object.hexdigest()
    return hex_digest


def anonymize_map(col, map_key):
    if hasattr(col, "__iter__"):
        parts = len(col)
        newcol = [{}] * parts
        for part_i in range(0, parts):
            part = col[part_i]
            if DEBUG != "false":
                logger.debug(
                    f"changing part {map_key} ({part[map_key]}) to ({anonymizer(part[map_key])})"
                )
            # assign back to map
            part[map_key] = anonymizer(part[map_key])
            newcol[part_i] = part
        return newcol
    elif isinstance(col, dict):
        newcol = {}
        newcol[map_key] = anonymizer(col[map_key])
        return newcol
    else:
        raise "Unsupported type"


def anonymizer(col):
    if isinstance(col, str):
        return generate_sha1_hash(col)
    if hasattr(col, "__iter__"):
        return [generate_sha1_hash(val) for val in col]
    else:
        raise Exception(f"Unmapped type for {type(col)}")


def main():
    p = Pool(N_PROC)

    if len(LISTS_TO_PARSE) > 0:
        p.map(parse_mail_at, LISTS_TO_PARSE)
    else:
        p.map(parse_mail_at, os.listdir(INPUT_DIR_PATH))


# for debugging only
def sequential():
    for mail_l in os.listdir(INPUT_DIR_PATH):
        parse_mail_at(mail_l)


if __name__ == "__main__":
    main()
