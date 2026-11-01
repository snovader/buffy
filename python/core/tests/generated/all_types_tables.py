"""The code generated from ``schemas/all_types.tafla``: how tests read its
messages back and describe them to build their tables by hand, and strategies
of their fields' values."""

from typing import Any

from hypothesis.strategies import SearchStrategy, builds, just, lists, recursive

from tests.descriptions import SCALAR_VALUES, STRINGS, scalar_values
from tests.generated.all_types.all_types import (
    Arrays,
    ArraysFields,
    Documented,
    Inner,
    InnerFields,
    Keywords,
    KeywordsFields,
    Nested,
    NestedFields,
    Outer,
    OuterFields,
    Packed,
    Reserved,
    ReservedFields,
    Scalars,
    ScalarsFields,
    Tree,
    TreeFields,
)
from tests.generated.descriptions import (
    message,
    scalar,
    string,
    struct_bytes,
    table,
    typed_array,
    vector,
)
from tests.tables import DEFAULT_LAYOUT, Description

# Reading every field, with arrays read from their views into lists to compare
# with the fields that were written


def read_scalars(scalars: Scalars) -> ScalarsFields:
    return {
        "int8": scalars.int8(),
        "uint8": scalars.uint8(),
        "int16": scalars.int16(),
        "uint16": scalars.uint16(),
        "int32": scalars.int32(),
        "uint32": scalars.uint32(),
        "int64": scalars.int64(),
        "uint64": scalars.uint64(),
        "float32": scalars.float32(),
        "float64": scalars.float64(),
    }


def read_arrays(arrays: Arrays) -> ArraysFields:
    return {
        "int8": arrays.int8().tolist(),
        "uint8": arrays.uint8().tolist(),
        "int16": arrays.int16().tolist(),
        "uint16": arrays.uint16().tolist(),
        "int32": arrays.int32().tolist(),
        "uint32": arrays.uint32().tolist(),
        "int64": arrays.int64().tolist(),
        "uint64": arrays.uint64().tolist(),
        "float32": arrays.float32().tolist(),
        "float64": arrays.float64().tolist(),
    }


def read_nested(nested: Nested) -> NestedFields:
    return {
        "label": nested.label(),
        "outer": _read_outer(nested.outer()),
        "trees": [_read_tree(tree) for tree in nested.trees()],
    }


def _read_outer(outer: Outer) -> OuterFields:
    return {"count": outer.count(), "inner": _read_inner(outer.inner())}


def _read_inner(inner: Inner) -> InnerFields:
    return {"name": inner.name(), "values": inner.values().tolist()}


def _read_tree(tree: Tree) -> TreeFields:
    return {
        "value": tree.value(),
        "children": [_read_tree(child) for child in tree.children()],
    }


def read_reserved(reserved: Reserved) -> ReservedFields:
    return {"keywords": _read_keywords(reserved.keywords())}


def _read_keywords(keywords: Keywords) -> KeywordsFields:
    return {
        "class_": keywords.class_(),
        "default": keywords.default(),
        "in_": keywords.in_(),
        "match_": keywords.match_(),
        "self": keywords.self(),
        "type_": keywords.type_(),
    }


# The tables that the generated code writes, in the conformance suite's model


def describe_scalars(fields: ScalarsFields) -> Description:
    return message(
        DEFAULT_LAYOUT,
        [
            scalar("int8", fields["int8"]),
            scalar("uint8", fields["uint8"]),
            scalar("int16", fields["int16"]),
            scalar("uint16", fields["uint16"]),
            scalar("int32", fields["int32"]),
            scalar("uint32", fields["uint32"]),
            scalar("int64", fields["int64"]),
            scalar("uint64", fields["uint64"]),
            scalar("float32", fields["float32"]),
            scalar("float64", fields["float64"]),
        ],
    )


def describe_arrays(fields: ArraysFields) -> Description:
    return message(
        DEFAULT_LAYOUT,
        [
            typed_array("int8", fields["int8"]),
            typed_array("uint8", fields["uint8"]),
            typed_array("int16", fields["int16"]),
            typed_array("uint16", fields["uint16"]),
            typed_array("int32", fields["int32"]),
            typed_array("uint32", fields["uint32"]),
            typed_array("int64", fields["int64"]),
            typed_array("uint64", fields["uint64"]),
            typed_array("float32", fields["float32"]),
            typed_array("float64", fields["float64"]),
        ],
    )


def describe_nested(fields: NestedFields) -> Description:
    outer = fields["outer"]
    inner = outer["inner"]

    return message(
        DEFAULT_LAYOUT,
        [
            string(fields["label"]),
            table(
                [
                    scalar("uint32", outer["count"]),
                    table(
                        [
                            string(inner["name"]),
                            typed_array("float64", inner["values"]),
                        ]
                    ),
                ]
            ),
            vector(_describe_tree(tree) for tree in fields["trees"]),
        ],
    )


def _describe_tree(tree: TreeFields) -> list[Description]:
    return [
        scalar("int32", tree["value"]),
        vector(_describe_tree(child) for child in tree["children"]),
    ]


def describe_reserved(fields: ReservedFields) -> Description:
    keywords = fields["keywords"]

    return message(
        DEFAULT_LAYOUT,
        [
            table(
                [
                    scalar("uint8", keywords["class_"]),
                    scalar("uint8", keywords["default"]),
                    scalar("uint8", keywords["in_"]),
                    scalar("uint8", keywords["match_"]),
                    scalar("uint8", keywords["self"]),
                    string(keywords["type_"]),
                ]
            )
        ],
    )


def describe_packed(packed: Packed) -> list[Description]:
    """The fields of a ``Packed``, in the order that they lie in its bytes,
    which is the order of the scalar types, as its fields are named"""
    return [scalar(kind, getattr(packed, kind)) for kind in SCALAR_VALUES]


def packed_bytes(packed: Packed) -> bytes:
    """A ``Packed`` as its fields lie at fixed offsets, little-endian, without
    padding"""
    return struct_bytes(describe_packed(packed))


def describe_documented(documented: Documented) -> list[Description]:
    """The fields of a ``Documented``, in the order that they lie in its
    bytes"""
    return [
        scalar("uint8", documented.one),
        scalar("uint16", documented.two),
        scalar("uint32", documented.undocumented),
    ]


def documented_bytes(documented: Documented) -> bytes:
    """A ``Documented`` as its fields lie at fixed offsets, little-endian,
    without padding"""
    return struct_bytes(describe_documented(documented))


# Values of the fields over each type's full range, with -0.0, NaN and the
# infinities, strings outside the Basic Multilingual Plane, empty arrays and
# vectors, and trees a few levels deep. The fields of Scalars, Arrays and
# Packed are named after their types, as SCALAR_VALUES names its strategies.

_SCALARS: dict[str, SearchStrategy[Any]] = {
    kind: scalar_values(kind) for kind in SCALAR_VALUES
}


def _arrays(values: SearchStrategy[Any]) -> SearchStrategy[list[Any]]:
    return lists(values, max_size=8)


scalars_fields: SearchStrategy[ScalarsFields] = builds(ScalarsFields, **_SCALARS)

arrays_fields: SearchStrategy[ArraysFields] = builds(
    ArraysFields,
    **{kind: _arrays(values) for kind, values in _SCALARS.items()},
)


def _trees_of(children: SearchStrategy[TreeFields]) -> SearchStrategy[TreeFields]:
    return builds(
        TreeFields, value=_SCALARS["int32"], children=lists(children, max_size=3)
    )


_leaves: SearchStrategy[TreeFields] = builds(
    TreeFields, value=_SCALARS["int32"], children=just([])
)
_trees: SearchStrategy[TreeFields] = recursive(_leaves, _trees_of, max_leaves=8)

nested_fields: SearchStrategy[NestedFields] = builds(
    NestedFields,
    label=STRINGS,
    outer=builds(
        OuterFields,
        count=_SCALARS["uint32"],
        inner=builds(InnerFields, name=STRINGS, values=_arrays(_SCALARS["float64"])),
    ),
    trees=lists(_trees, max_size=3),
)

reserved_fields: SearchStrategy[ReservedFields] = builds(
    ReservedFields,
    keywords=builds(
        KeywordsFields,
        class_=_SCALARS["uint8"],
        default=_SCALARS["uint8"],
        in_=_SCALARS["uint8"],
        match_=_SCALARS["uint8"],
        self=_SCALARS["uint8"],
        type_=STRINGS,
    ),
)

packed_structs: SearchStrategy[Packed] = builds(Packed, **_SCALARS)

documented_structs: SearchStrategy[Documented] = builds(
    Documented,
    one=_SCALARS["uint8"],
    two=_SCALARS["uint16"],
    undocumented=_SCALARS["uint32"],
)
