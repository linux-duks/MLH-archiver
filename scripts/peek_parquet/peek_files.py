#!/usr/bin/env python3
"""
Peek into Parquet files or directories.

Usage:
    peek-files <path>
    peek-files <path> --select-by-column <value> [--column <name>]

Displays:
    - DataFrame preview (df.show())
    - Total row count
    - Row count per partition (if hive-partitioned)
    - Schema
    - Row lookup by column value (--select-by-column)
"""

import sys
from pathlib import Path

import polars as pl


def parse_args(args: list[str]) -> dict:
    """Parse command line arguments and return a dict of options."""
    if not args:
        print("Usage: peek-files <path>")
        print("       peek-files <path> --select-by-column <value> [--column <name>]")
        print("  path:              Path to a parquet file or directory")
        print("  --select-by-column: Value to search for")
        print("  --column:           Column name to search in (default: email_id)")
        sys.exit(1)

    result = {"path": None, "select_value": None, "select_column": "email_id"}

    i = 0
    positional = []
    while i < len(args):
        if args[i] == "--dir":
            i += 1
            if i >= len(args):
                print("Error: --dir requires a path argument")
                sys.exit(1)
            result["path"] = args[i]
        elif args[i] == "--select-by-column":
            i += 1
            if i >= len(args):
                print("Error: --select-by-column requires a value argument")
                sys.exit(1)
            result["select_value"] = args[i]
        elif args[i] == "--column":
            i += 1
            if i >= len(args):
                print("Error: --column requires a name argument")
                sys.exit(1)
            result["select_column"] = args[i]
        else:
            positional.append(args[i])
        i += 1

    if result["path"] is None and positional:
        result["path"] = positional[0]

    if result["path"] is None:
        print("Error: path argument is required")
        sys.exit(1)

    return result


def expand_path(path_str: str) -> Path:
    """Expand and resolve the given path to an absolute path."""
    path = Path(path_str).expanduser()
    if not path.is_absolute():
        path = Path.cwd() / path
    return path.resolve()


def find_parquet_files(path: Path) -> list[Path]:
    """Find all parquet files in the given path."""
    if path.is_file():
        if path.suffix in (".parquet", ".pq"):
            return [path]
        return []
    elif path.is_dir():
        return sorted(path.rglob("*.parquet"))
    else:
        return []


def detect_partitions(path: Path, parquet_files: list[Path]) -> dict[str, Path] | None:
    """
    Detect hive partitioning from directory structure.
    Returns dict mapping partition value to directory if hive-partitioned.
    """
    if not parquet_files:
        return None

    # Check if files are in hive-partitioned directories
    # e.g., mailing_list=foo/part-0.parquet
    partitions = {}
    for pf in parquet_files:
        parent = pf.parent
        if "=" in parent.name:
            # This looks like a hive partition
            partition_key = parent.name
            partitions[partition_key] = parent

    return partitions if partitions else None


def show_partition_stats(parquet_files: list[Path]) -> None:
    """Show row count per partition."""
    print("\nPartition Statistics:")
    print("-" * 50)

    # Group files by their parent directory (partition)
    partition_groups: dict[Path, list[Path]] = {}
    for pf in parquet_files:
        parent = pf.parent
        if parent not in partition_groups:
            partition_groups[parent] = []
        partition_groups[parent].append(pf)

    for partition_dir, files in sorted(partition_groups.items()):
        partition_name = partition_dir.name
        total_rows = 0

        for pf in files:
            try:
                # Use scan for efficiency
                lf = pl.scan_parquet(pf)
                count = lf.select(pl.len()).collect().item()
                total_rows += count
            except Exception as e:
                print(f"  Warning: Could not read {pf.name}: {e}")

        print(f"  {partition_name}: {total_rows:,} rows ({len(files)} file(s))")

    total = sum(
        pl.scan_parquet(pf).select(pl.len()).collect().item() for pf in parquet_files
    )
    print("-" * 50)
    print(f"  TOTAL: {total:,} rows across {len(parquet_files)} file(s)")


def show_file_info(path: Path) -> None:
    """Show info for a single parquet file."""
    print(f"\nFile: {path}")
    print("-" * 50)

    df = pl.read_parquet(path)
    print(f"\nShape: {df.shape[0]:,} rows x {df.shape[1]} columns")

    print("\nSchema:")
    for col, dtype in df.schema.items():
        print(f"  {col}: {dtype}")

    print("\nPreview (df.show()):")
    df.show()


def show_directory_info(path: Path, parquet_files: list[Path]) -> None:
    """Show info for a directory of parquet files."""
    print(f"\nDirectory: {path}")
    print(f"Found {len(parquet_files)} parquet file(s)")
    print("-" * 50)

    # Check for hive partitioning
    partitions = detect_partitions(path, parquet_files)

    if partitions:
        print("\nDetected hive partitioning")
        show_partition_stats(parquet_files)
    else:
        # No partitioning, just show total
        total_rows = sum(
            pl.scan_parquet(pf).select(pl.len()).collect().item()
            for pf in parquet_files
        )
        print(f"\nTotal: {total_rows:,} rows across {len(parquet_files)} file(s)")

    # Show schema from first file
    if parquet_files:
        print("\nSchema (from first file):")
        df_sample = pl.read_parquet(parquet_files[0], n_rows=0)
        for col, dtype in df_sample.schema.items():
            print(f"  {col}: {dtype}")

        # Show preview
        print("\nPreview (first 10 rows from first file):")
        df_preview = pl.read_parquet(parquet_files[0], n_rows=10)
        df_preview.show()


def select_by_column(parquet_files: list[Path], column: str, value: str) -> None:
    """Search across all parquet files for rows matching column=value and print them."""
    print(f"\nSearching for {column}='{value}' across {len(parquet_files)} file(s)...")
    print()

    found = 0
    for pf in parquet_files:
        try:
            df = pl.read_parquet(pf)
        except Exception as e:
            print(f"  Warning: Could not read {pf}: {e}")
            continue

        if column not in df.columns:
            continue

        mask = df[column].cast(pl.Utf8) == value
        matches = df.filter(mask)

        if matches.is_empty():
            continue

        for row in matches.iter_rows(named=True):
            if found > 0:
                print("------")
            for col_name, col_value in row.items():
                print(f"{col_name}: {col_value!r}")
            found += 1

    if found == 0:
        print(f"No rows found with {column}='{value}'")
    else:
        print()
        print(f"Found {found} matching row(s)")


def main():
    opts = parse_args(sys.argv[1:])
    path_str = opts["path"]
    path = expand_path(path_str)

    print(f"Inspecting: {path}")

    if not path.exists():
        print(f"Error: Path does not exist: {path}")
        sys.exit(1)

    parquet_files = find_parquet_files(path)

    if not parquet_files:
        print(f"No parquet files found at: {path}")
        sys.exit(1)

    if opts["select_value"] is not None:
        select_by_column(parquet_files, opts["select_column"], opts["select_value"])
        return

    if len(parquet_files) == 1 and path.is_file():
        show_file_info(path)
    else:
        show_directory_info(path, parquet_files)


if __name__ == "__main__":
    main()
