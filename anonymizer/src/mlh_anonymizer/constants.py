"""Constant definitions for the anonymizer.

This module contains column definitions and other constants
that define the anonymization schema.
"""

# Default maximum number of processes (used by configs.py)
N_PROC_DEFAULT_MAX = 16

# Columns to anonymize with direct SHA-1 hashing
ANONYMIZE_COLUMNS = [
    "from",
    "to",
    "cc",
]

# Generate a sub-dataset with a mapping of values for these columns
SPLIT_DATASET_COLUMNS = ["from"]

# Columns with nested structures to anonymize (dot notation)
# Format: "parent.child" where child is the key to anonymize
ANONYMIZE_MAP = [
    "trailers.identification",
]
