from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Record(_message.Message):
    __slots__ = ("x", "y", "id", "name")
    X_FIELD_NUMBER: _ClassVar[int]
    Y_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    x: float
    y: float
    id: int
    name: str
    def __init__(self, x: _Optional[float] = ..., y: _Optional[float] = ..., id: _Optional[int] = ..., name: _Optional[str] = ...) -> None: ...

class Columns(_message.Message):
    __slots__ = ("base", "keys", "a", "b")
    BASE_FIELD_NUMBER: _ClassVar[int]
    KEYS_FIELD_NUMBER: _ClassVar[int]
    A_FIELD_NUMBER: _ClassVar[int]
    B_FIELD_NUMBER: _ClassVar[int]
    base: int
    keys: _containers.RepeatedScalarFieldContainer[int]
    a: _containers.RepeatedScalarFieldContainer[float]
    b: _containers.RepeatedScalarFieldContainer[float]
    def __init__(self, base: _Optional[int] = ..., keys: _Optional[_Iterable[int]] = ..., a: _Optional[_Iterable[float]] = ..., b: _Optional[_Iterable[float]] = ...) -> None: ...

class Document(_message.Message):
    __slots__ = ("x", "y", "name", "columns")
    X_FIELD_NUMBER: _ClassVar[int]
    Y_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    COLUMNS_FIELD_NUMBER: _ClassVar[int]
    x: float
    y: float
    name: str
    columns: Columns
    def __init__(self, x: _Optional[float] = ..., y: _Optional[float] = ..., name: _Optional[str] = ..., columns: _Optional[_Union[Columns, _Mapping]] = ...) -> None: ...

class Pair(_message.Message):
    __slots__ = ("first", "second")
    FIRST_FIELD_NUMBER: _ClassVar[int]
    SECOND_FIELD_NUMBER: _ClassVar[int]
    first: float
    second: float
    def __init__(self, first: _Optional[float] = ..., second: _Optional[float] = ...) -> None: ...

class Item(_message.Message):
    __slots__ = ("value", "pair")
    VALUE_FIELD_NUMBER: _ClassVar[int]
    PAIR_FIELD_NUMBER: _ClassVar[int]
    value: float
    pair: Pair
    def __init__(self, value: _Optional[float] = ..., pair: _Optional[_Union[Pair, _Mapping]] = ...) -> None: ...

class List(_message.Message):
    __slots__ = ("items",)
    ITEMS_FIELD_NUMBER: _ClassVar[int]
    items: _containers.RepeatedCompositeFieldContainer[Item]
    def __init__(self, items: _Optional[_Iterable[_Union[Item, _Mapping]]] = ...) -> None: ...

class Sample(_message.Message):
    __slots__ = ("key", "a", "b")
    KEY_FIELD_NUMBER: _ClassVar[int]
    A_FIELD_NUMBER: _ClassVar[int]
    B_FIELD_NUMBER: _ClassVar[int]
    key: int
    a: float
    b: float
    def __init__(self, key: _Optional[int] = ..., a: _Optional[float] = ..., b: _Optional[float] = ...) -> None: ...
