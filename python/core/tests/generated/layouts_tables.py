"""The code generated from ``schemas/layouts.buffy``, of the same fields in
every layout: how tests read its messages back and describe them to build
their tables by hand, and strategies of their fields' values."""

from collections.abc import Buffer
from dataclasses import dataclass

from hypothesis.strategies import SearchStrategy, builds, integers, lists

from tests.descriptions import STRINGS, scalar_values
from tests.generated.descriptions import layout, message, scalar, string, typed_array
from tests.generated.layouts.layouts import (
    Offset8,
    Offset8Sized,
    Offset8SizedVersioned,
    Offset8Versioned,
    Offset16,
    Offset16Sized,
    Offset16SizedVersioned,
    Offset16Versioned,
    Offset32,
    Offset32Fields,
    Offset32Sized,
    Offset32SizedVersioned,
    Offset32Versioned,
)
from tests.tables import Description

type LayoutsMessage = (
    Offset8
    | Offset8Sized
    | Offset8Versioned
    | Offset8SizedVersioned
    | Offset16
    | Offset16Sized
    | Offset16Versioned
    | Offset16SizedVersioned
    | Offset32
    | Offset32Sized
    | Offset32Versioned
    | Offset32SizedVersioned
)

# The fields of every message, whose own ``Fields`` types are the same
type Fields = Offset32Fields


@dataclass(frozen=True)
class MessageLayout:
    """A message, with the layout and version of its attributes"""

    message: type[LayoutsMessage]
    layout: Description
    version: int | None = None

    def offset_size(self) -> int:
        return self.layout["offsetSize"]

    def max_value(self) -> int:
        """The largest version, offset or length that its offsets hold, which
        is also the longest table that they address"""
        return 2 ** (8 * self.offset_size()) - 1

    def other_versions(self) -> SearchStrategy[int]:
        """The versions that its offsets hold but its own"""
        return integers(0, self.max_value()).filter(
            lambda version: version != self.version
        )

    def read(self, data: Buffer) -> Fields:
        """Reads every field of the message from ``data``, with its values read
        from their view into a list to compare with the fields that were
        written"""
        message = self.message(data)

        return {
            "id": message.id(),
            "name": message.name(),
            "values": message.values().tolist(),
        }

    def describe(self, fields: Fields, version: int | None = None) -> Description:
        """Describes a table of the message, in its version by default"""
        return message(
            self.layout,
            [
                scalar("uint32", fields["id"]),
                string(fields["name"]),
                typed_array("uint16", fields["values"]),
            ],
            self.version if version is None else version,
        )


MESSAGES: list[MessageLayout] = [
    MessageLayout(Offset8, layout(offset_size=1)),
    MessageLayout(Offset8Sized, layout(sized=True, offset_size=1)),
    MessageLayout(Offset8Versioned, layout(versioned=True, offset_size=1), 255),
    MessageLayout(
        Offset8SizedVersioned, layout(sized=True, versioned=True, offset_size=1), 0
    ),
    MessageLayout(Offset16, layout(offset_size=2)),
    MessageLayout(Offset16Sized, layout(sized=True, offset_size=2)),
    MessageLayout(Offset16Versioned, layout(versioned=True, offset_size=2), 65535),
    MessageLayout(
        Offset16SizedVersioned, layout(sized=True, versioned=True, offset_size=2), 1
    ),
    MessageLayout(Offset32, layout()),
    MessageLayout(Offset32Sized, layout(sized=True)),
    MessageLayout(Offset32Versioned, layout(versioned=True), 4294967295),
    MessageLayout(Offset32SizedVersioned, layout(sized=True, versioned=True), 2),
]
"""Every message of layouts.buffy, with the layouts and versions that its
attributes give"""


# Values of the fields, which fit tables of 8-bit offsets
fields_of_every_message: SearchStrategy[Fields] = builds(
    Offset32Fields,
    id=scalar_values("uint32"),
    name=STRINGS,
    values=lists(scalar_values("uint16"), max_size=8),
)
