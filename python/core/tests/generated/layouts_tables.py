"""The code generated from ``schemas/layouts.tafla``, of the same fields in
every layout: how tests read its messages back and describe them to build
their tables by hand, and strategies of their fields' values."""

from collections.abc import Buffer
from dataclasses import dataclass

from hypothesis.strategies import SearchStrategy, builds, integers, lists

from tests.descriptions import STRINGS, scalar_values
from tests.generated.descriptions import layout, message, scalar, string, typed_array
from tests.generated.layouts.layouts import (
    Uint8Offsets,
    Uint8OffsetsSized,
    Uint8OffsetsSizedVersioned,
    Uint8OffsetsVersioned,
    Uint16Offsets,
    Uint16OffsetsSized,
    Uint16OffsetsSizedVersioned,
    Uint16OffsetsVersioned,
    Uint32Offsets,
    Uint32OffsetsFields,
    Uint32OffsetsSized,
    Uint32OffsetsSizedVersioned,
    Uint32OffsetsVersioned,
)
from tests.tables import LONGEST, Description, OffsetType

type LayoutsMessage = (
    Uint8Offsets
    | Uint8OffsetsSized
    | Uint8OffsetsVersioned
    | Uint8OffsetsSizedVersioned
    | Uint16Offsets
    | Uint16OffsetsSized
    | Uint16OffsetsVersioned
    | Uint16OffsetsSizedVersioned
    | Uint32Offsets
    | Uint32OffsetsSized
    | Uint32OffsetsVersioned
    | Uint32OffsetsSizedVersioned
)

# The fields of every message, whose own ``Fields`` types are the same
type Fields = Uint32OffsetsFields


@dataclass(frozen=True)
class MessageLayout:
    """A message, with the layout and version of its attributes"""

    message: type[LayoutsMessage]
    layout: Description
    version: int | None = None

    def offset_type(self) -> OffsetType:
        return self.layout["offsetType"]

    def max_value(self) -> int:
        """The largest version, offset or length that its offsets hold, which
        is also the longest table that they address"""
        return LONGEST[self.offset_type()]

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
    MessageLayout(Uint8Offsets, layout(offset_type="uint8")),
    MessageLayout(Uint8OffsetsSized, layout(sized=True, offset_type="uint8")),
    MessageLayout(
        Uint8OffsetsVersioned, layout(versioned=True, offset_type="uint8"), 255
    ),
    MessageLayout(
        Uint8OffsetsSizedVersioned,
        layout(sized=True, versioned=True, offset_type="uint8"),
        0,
    ),
    MessageLayout(Uint16Offsets, layout(offset_type="uint16")),
    MessageLayout(Uint16OffsetsSized, layout(sized=True, offset_type="uint16")),
    MessageLayout(
        Uint16OffsetsVersioned, layout(versioned=True, offset_type="uint16"), 65535
    ),
    MessageLayout(
        Uint16OffsetsSizedVersioned,
        layout(sized=True, versioned=True, offset_type="uint16"),
        1,
    ),
    MessageLayout(Uint32Offsets, layout()),
    MessageLayout(Uint32OffsetsSized, layout(sized=True)),
    MessageLayout(Uint32OffsetsVersioned, layout(versioned=True), 4294967295),
    MessageLayout(Uint32OffsetsSizedVersioned, layout(sized=True, versioned=True), 2),
]
"""Every message of layouts.tafla, with the layouts and versions that its
attributes give"""


# Values of the fields, which fit tables of uint8 offsets
fields_of_every_message: SearchStrategy[Fields] = builds(
    Uint32OffsetsFields,
    id=scalar_values("uint32"),
    name=STRINGS,
    values=lists(scalar_values("uint16"), max_size=8),
)
