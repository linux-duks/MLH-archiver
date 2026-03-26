"""SHA-1 hash generation for anonymization."""

import hashlib


def generate_sha1_hash(input_string: str) -> str:
    """Generate SHA-1 hash of input string.

    Args:
        input_string: String to hash

    Returns:
        Hexadecimal SHA-1 digest (40 characters)
    """
    encoded_string = input_string.encode("utf-8")
    sha1_hash_object = hashlib.sha1()
    sha1_hash_object.update(encoded_string)
    return sha1_hash_object.hexdigest()
