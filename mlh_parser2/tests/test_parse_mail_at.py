"""
Integration tests for parse_mail_at function.

These tests verify that parse_mail_at correctly processes all .eml files
in a directory and produces a parquet file with the expected number of rows.
"""

import shutil
import tempfile
from pathlib import Path

import polars as pl
import pytest

from mlh_parser.parser import parse_mail_at
from mlh_parser.constants import PARQUET_COLS_SCHEMA


# Test directory paths (relative to this test file)
TESTS_DIR = Path(__file__).parent.resolve()
COMPLETE_CASES_DIR = TESTS_DIR / "complete_cases"
DATE_CASES_DIR = TESTS_DIR / "date_cases"
SYNTHETIC_DIR = TESTS_DIR / "synthetic"


@pytest.fixture
def temp_output_dir():
    """Create a temporary output directory for test results."""
    temp_dir = tempfile.mkdtemp(prefix="mlh_parser_test_")
    yield temp_dir
    # Cleanup after test
    shutil.rmtree(temp_dir, ignore_errors=True)


def count_eml_files(directory: Path) -> int:
    """Count the number of .eml files in a directory."""
    return len([f for f in directory.iterdir() if f.suffix == ".eml" and f.is_file()])


def get_mailing_list_name(directory: Path) -> str:
    """Generate a mailing list name from the directory name."""
    return directory.name


class TestParseMailAtCompleteCases:
    """Integration tests for parse_mail_at using complete_cases directory."""

    @pytest.fixture(autouse=True)
    def setup_test_dirs(self, temp_output_dir):
        """Set up test directories for complete_cases tests."""
        self.input_dir = tempfile.mkdtemp(prefix="mlh_parser_input_")
        self.output_dir = temp_output_dir
        self.mailing_list = get_mailing_list_name(COMPLETE_CASES_DIR)

        # Create mailing list subdirectory in input
        list_input_dir = Path(self.input_dir) / self.mailing_list
        list_input_dir.mkdir(parents=True, exist_ok=True)

        # Copy all .eml files to input directory
        for eml_file in COMPLETE_CASES_DIR.glob("*.eml"):
            shutil.copy2(eml_file, list_input_dir / eml_file.name)

        yield

        # Cleanup input directory
        shutil.rmtree(self.input_dir, ignore_errors=True)

    def test_parse_complete_cases_produces_correct_row_count(self):
        """Test that parse_mail_at produces a parquet file with correct row count."""
        # Count expected .eml files
        expected_count = count_eml_files(COMPLETE_CASES_DIR)
        assert expected_count > 0, "No .eml files found in complete_cases directory"

        # Run the parser
        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        # Locate the output parquet file
        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        # Verify parquet file exists
        assert parquet_path.exists(), f"Parquet file not found at {parquet_path}"

        # Read parquet and verify row count
        df = pl.read_parquet(parquet_path)
        actual_count = len(df)

        assert actual_count == expected_count, (
            f"Parquet file has {actual_count} rows, expected {expected_count} "
            f"(matching .eml file count)"
        )

    def test_parse_complete_cases_schema(self):
        """Test that the output parquet file has the expected schema."""
        # Run the parser
        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        # Locate the output parquet file
        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        # Read parquet schema
        df = pl.read_parquet(parquet_path)

        # Verify expected columns exist
        expected_columns = PARQUET_COLS_SCHEMA.keys()

        assert len(df.columns) == len(expected_columns), "Numer of columns missmatch"
        for col in expected_columns:
            assert col in df.columns, f"Missing expected column: {col}"


class TestParseMailAtDateCases:
    """Integration tests for parse_mail_at using date_cases directory."""

    @pytest.fixture(autouse=True)
    def setup_test_dirs(self, temp_output_dir):
        """Set up test directories for date_cases tests."""
        self.input_dir = tempfile.mkdtemp(prefix="mlh_parser_input_")
        self.output_dir = temp_output_dir
        self.mailing_list = get_mailing_list_name(DATE_CASES_DIR)

        # Create mailing list subdirectory in input
        list_input_dir = Path(self.input_dir) / self.mailing_list
        list_input_dir.mkdir(parents=True, exist_ok=True)

        # Copy all .eml files to input directory
        for eml_file in DATE_CASES_DIR.glob("*.eml"):
            shutil.copy2(eml_file, list_input_dir / eml_file.name)

        yield

        # Cleanup input directory
        shutil.rmtree(self.input_dir, ignore_errors=True)

    def test_parse_date_cases_produces_correct_row_count(self):
        """Test that parse_mail_at produces a parquet file with correct row count."""
        # Count expected .eml files
        expected_count = count_eml_files(DATE_CASES_DIR)
        assert expected_count > 0, "No .eml files found in date_cases directory"

        # Run the parser
        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        # Locate the output parquet file
        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        # Verify parquet file exists
        assert parquet_path.exists(), f"Parquet file not found at {parquet_path}"

        # Read parquet and verify row count
        df = pl.read_parquet(parquet_path)
        actual_count = len(df)

        assert actual_count == expected_count, (
            f"Parquet file has {actual_count} rows, expected {expected_count} "
            f"(matching .eml file count)"
        )

    def test_parse_date_cases_dates_are_parsed(self):
        """Test that dates are correctly parsed in the output."""
        # Run the parser
        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        # Locate the output parquet file
        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        # Read parquet
        df = pl.read_parquet(parquet_path)

        # Verify date column exists and has datetime type
        assert "date" in df.columns, "Missing 'date' column"

        # Check that at least some dates are not null
        # (some emails may have unparseable dates)
        non_null_dates = df.filter(pl.col("date").is_not_null())
        assert len(non_null_dates) > 0, (
            "All dates are null, date parsing may have failed"
        )


class TestParseMailAtEmptyDirectory:
    """Edge case tests for parse_mail_at."""

    @pytest.fixture(autouse=True)
    def setup_test_dirs(self, temp_output_dir):
        """Set up empty test directory."""
        self.input_dir = tempfile.mkdtemp(prefix="mlh_parser_input_")
        self.output_dir = temp_output_dir
        self.mailing_list = "empty_test_list"

        # Create empty mailing list subdirectory
        list_input_dir = Path(self.input_dir) / self.mailing_list
        list_input_dir.mkdir(parents=True, exist_ok=True)

        yield

        # Cleanup input directory
        shutil.rmtree(self.input_dir, ignore_errors=True)

    def test_parse_empty_directory(self):
        """Test that parse_mail_at handles empty directories gracefully."""
        # Run the parser on empty directory
        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        # Locate the output parquet file
        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        # Verify parquet file exists
        assert parquet_path.exists(), f"Parquet file not found at {parquet_path}"

        # Read parquet and verify it's empty
        df = pl.read_parquet(parquet_path)
        assert len(df) == 0, "Expected empty parquet file for empty input directory"


class TestParseMailAtParquetInput:
    """Tests for parse_mail_at with .parquet files mixed into the input directory."""

    @pytest.fixture(autouse=True)
    def setup_test_dirs(self, temp_output_dir):
        """Set up test directories and create input parquet from synthetic .eml files."""
        self.input_dir = tempfile.mkdtemp(prefix="mlh_parser_input_")
        self.output_dir = temp_output_dir
        self.mailing_list = "synthetic_parquet"

        # Create mailing list subdirectory in input
        list_input_dir = Path(self.input_dir) / self.mailing_list
        list_input_dir.mkdir(parents=True, exist_ok=True)

        # Build a parquet file from the synthetic .eml files
        rows = []
        for eml_file in sorted(SYNTHETIC_DIR.glob("*.eml")):
            content = eml_file.read_text(encoding="utf-8")
            rows.append({"email_id": eml_file.stem, "content": content})

        df = pl.DataFrame(rows, schema={"email_id": pl.String, "content": pl.String})
        self.parquet_file_name = "data_000.parquet"
        parquet_input_path = list_input_dir / self.parquet_file_name
        df.write_parquet(parquet_input_path)

        self.expected_count = len(rows)

        yield

        shutil.rmtree(self.input_dir, ignore_errors=True)

    def test_parse_parquet_input_produces_correct_row_count(self):
        """Test that parse_mail_at with parquet input produces correct row count."""
        assert self.expected_count > 0, "No .eml files found in synthetic directory"

        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        assert parquet_path.exists(), f"Parquet file not found at {parquet_path}"

        df = pl.read_parquet(parquet_path)
        assert len(df) == self.expected_count, (
            f"Parquet file has {len(df)} rows, expected {self.expected_count}"
        )

    def test_parse_parquet_input_schema(self):
        """Test that output parquet from parquet input has expected schema and data."""
        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        df = pl.read_parquet(parquet_path)

        expected_columns = PARQUET_COLS_SCHEMA.keys()
        assert len(df.columns) == len(expected_columns), "Number of columns mismatch"
        for col in expected_columns:
            assert col in df.columns, f"Missing expected column: {col}"

        # Verify __file_name values use email_id:parquet_file_name format
        file_names = df["__file_name"].to_list()
        expected_names = sorted(
            [f"{f.stem}:{self.parquet_file_name}" for f in SYNTHETIC_DIR.glob("*.eml")]
        )
        assert sorted(file_names) == expected_names, (
            "__file_name values don't match email_id:parquet_filename format"
        )

    def test_parse_parquet_input_handles_empty_parquet(self):
        """Test that an empty parquet file produces an empty output."""
        # Override with empty parquet
        empty_df = pl.DataFrame(schema={"email_id": pl.String, "content": pl.String})
        list_input_dir = Path(self.input_dir) / self.mailing_list
        parquet_input_path = list_input_dir / self.parquet_file_name
        empty_df.write_parquet(parquet_input_path)

        parse_mail_at(
            mailing_list=self.mailing_list,
            input_dir_path=self.input_dir,
            output_dir_path=self.output_dir,
            fail_on_parsing_error=True,
        )

        parquet_path = (
            Path(self.output_dir)
            / "parsed"
            / f"list={self.mailing_list}"
            / "list_data.parquet"
        )

        assert parquet_path.exists(), f"Parquet file not found at {parquet_path}"
        df = pl.read_parquet(parquet_path)
        assert len(df) == 0, "Expected empty parquet file for empty parquet input"

    def test_parse_parquet_input_invalid_schema_raises(self):
        """Test that parquet with missing required columns raises KeyError."""
        bad_df = pl.DataFrame({"wrong_col": ["val1", "val2"]})
        list_input_dir = Path(self.input_dir) / self.mailing_list
        parquet_input_path = list_input_dir / self.parquet_file_name
        bad_df.write_parquet(parquet_input_path)

        with pytest.raises(KeyError):
            parse_mail_at(
                mailing_list=self.mailing_list,
                input_dir_path=self.input_dir,
                output_dir_path=self.output_dir,
                fail_on_parsing_error=True,
            )
