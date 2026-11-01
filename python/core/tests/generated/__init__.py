"""Tests of the code that buffy-gen generates from the example schemas, which
lies in a directory of each schema's name. The generator's golden test owns
those directories."""

from hypothesis import settings

# Half of Hypothesis's default examples, which buffy_core's own tests run: the
# generated code calls buffy_core, and a mistake of its own, such as a field of
# the wrong type or index, shows in any table
SETTINGS = settings(max_examples=50)
