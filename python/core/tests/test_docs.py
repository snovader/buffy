import doctest
import unittest

import buffy_core


def load_tests(
    loader: unittest.TestLoader, tests: unittest.TestSuite, pattern: str | None
) -> unittest.TestSuite:
    """Runs the example in the package's docstring as written"""
    tests.addTests(doctest.DocTestSuite(buffy_core))
    return tests


if __name__ == "__main__":
    unittest.main()
