import doctest
import re
import unittest
from pathlib import Path

import buffy_core

# The package's README, which PyPI shows as its page
README = Path(__file__).parent.parent / "README.md"


def load_tests(
    loader: unittest.TestLoader, tests: unittest.TestSuite, pattern: str | None
) -> unittest.TestSuite:
    """Runs the example in the package's docstring as written"""
    tests.addTests(doctest.DocTestSuite(buffy_core))
    return tests


def python_blocks(markdown: str) -> str:
    """The code of the Markdown's Python blocks, in order"""
    blocks = re.findall(r"^```python\n(.*?)^```$", markdown, re.MULTILINE | re.DOTALL)
    return "\n".join(blocks)


class TestReadme(unittest.TestCase):
    def test_example_reads_back_the_table_it_wrote(self) -> None:
        example = python_blocks(README.read_text(encoding="utf-8"))
        namespace: dict[str, object] = {}

        # The README's own code, run as a reader would run it
        exec(compile(example, README, "exec"), namespace)  # noqa: S102

        self.assertEqual(namespace["latitude"], 59.3293)
        self.assertEqual(namespace["time_zone"], "Europe/Stockholm")
        values = namespace["values"]
        assert isinstance(values, memoryview)
        self.assertEqual(values.tolist(), [21.75, 18.5])


if __name__ == "__main__":
    unittest.main()
