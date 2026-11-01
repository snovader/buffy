use std::borrow::Cow;

use prost::Message;

use crate::codec::{Codec, DocumentWorkload, Error, Workload};
use crate::workloads::{
    Document, DocumentShape, Item, List, ListShape, Record, RecordShape, Sample, SampleShape,
    sum_column,
};

/// protobuf's messages, derived by hand in the shape that prost-build 0.14
/// generates from this schema, so that no protoc is needed. proto3 packs
/// repeated scalars, as prost does by default.
///
/// ```proto
/// syntax = "proto3";
///
/// message Record { double x = 1; double y = 2; int64 id = 3; string name = 4; }
///
/// message Columns {
///     int64 base = 1;
///     repeated uint32 keys = 2;
///     repeated float a = 3;
///     repeated float b = 4;
/// }
///
/// message Document {
///     double x = 1;
///     double y = 2;
///     string name = 3;
///     Columns columns = 4;
/// }
///
/// message Pair { float first = 1; float second = 2; }
/// message Item { float value = 1; Pair pair = 2; }
/// message List { repeated Item items = 1; }
///
/// message Sample { uint32 key = 1; float a = 2; float b = 3; }
/// ```
pub mod messages {
    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Record {
        #[prost(double, tag = "1")]
        pub x: f64,
        #[prost(double, tag = "2")]
        pub y: f64,
        #[prost(int64, tag = "3")]
        pub id: i64,
        #[prost(string, tag = "4")]
        pub name: ::prost::alloc::string::String,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Columns {
        #[prost(int64, tag = "1")]
        pub base: i64,
        #[prost(uint32, repeated, tag = "2")]
        pub keys: ::prost::alloc::vec::Vec<u32>,
        #[prost(float, repeated, tag = "3")]
        pub a: ::prost::alloc::vec::Vec<f32>,
        #[prost(float, repeated, tag = "4")]
        pub b: ::prost::alloc::vec::Vec<f32>,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct Document {
        #[prost(double, tag = "1")]
        pub x: f64,
        #[prost(double, tag = "2")]
        pub y: f64,
        #[prost(string, tag = "3")]
        pub name: ::prost::alloc::string::String,
        #[prost(message, optional, tag = "4")]
        pub columns: ::core::option::Option<Columns>,
    }

    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Pair {
        #[prost(float, tag = "1")]
        pub first: f32,
        #[prost(float, tag = "2")]
        pub second: f32,
    }

    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Item {
        #[prost(float, tag = "1")]
        pub value: f32,
        #[prost(message, optional, tag = "2")]
        pub pair: ::core::option::Option<Pair>,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct List {
        #[prost(message, repeated, tag = "1")]
        pub items: ::prost::alloc::vec::Vec<Item>,
    }

    #[derive(Clone, Copy, PartialEq, ::prost::Message)]
    pub struct Sample {
        #[prost(uint32, tag = "1")]
        pub key: u32,
        #[prost(float, tag = "2")]
        pub a: f32,
        #[prost(float, tag = "3")]
        pub b: f32,
    }
}

/// prost's messages, used as its documentation shows: built from the input
/// and written with `encode_to_vec()`, which sizes the bytes first, and read
/// with `decode()`.
///
/// Messages own their strings and repeated fields, so building one from the
/// borrowed input copies them, as a caller that keeps its values would, and
/// reading one copies them out of the bytes. A nested message that the bytes
/// leave out reads as `None`, which is an error here, since every workload
/// has them.
#[derive(Clone, Copy, Debug)]
pub struct Prost;

impl Codec for Prost {
    const NAME: &'static str = "prost";
}

impl Workload<RecordShape> for Prost {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Record<'_>) -> Result<Vec<u8>, Error> {
        let message = messages::Record {
            x: input.x,
            y: input.y,
            id: input.id,
            name: input.name.to_string(),
        };

        Ok(message.encode_to_vec())
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Record<'_>, Error> {
        let message = messages::Record::decode(bytes)?;

        Ok(Record {
            x: message.x,
            y: message.y,
            id: message.id,
            name: Cow::Owned(message.name),
        })
    }
}

impl Workload<DocumentShape> for Prost {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Document<'_>) -> Result<Vec<u8>, Error> {
        let columns = messages::Columns {
            base: input.base,
            keys: input.keys.to_vec(),
            a: input.a.to_vec(),
            b: input.b.to_vec(),
        };
        let message = messages::Document {
            x: input.x,
            y: input.y,
            name: input.name.to_string(),
            columns: Some(columns),
        };

        Ok(message.encode_to_vec())
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Document<'_>, Error> {
        let message = messages::Document::decode(bytes)?;
        let columns = message.columns.ok_or("The document has no columns")?;

        Ok(Document {
            x: message.x,
            y: message.y,
            name: Cow::Owned(message.name),
            base: columns.base,
            keys: Cow::Owned(columns.keys),
            a: Cow::Owned(columns.a),
            b: Cow::Owned(columns.b),
        })
    }
}

impl DocumentWorkload for Prost {
    fn sum_a(self, bytes: &[u8]) -> Result<f64, Error> {
        let message = messages::Document::decode(bytes)?;
        let columns = message.columns.ok_or("The document has no columns")?;

        Ok(sum_column(columns.a.iter().copied()))
    }
}

impl Workload<ListShape> for Prost {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &List) -> Result<Vec<u8>, Error> {
        let message = messages::List {
            items: input.items.iter().map(item_message).collect(),
        };

        Ok(message.encode_to_vec())
    }

    fn deserialize(self, bytes: &[u8]) -> Result<List, Error> {
        let message = messages::List::decode(bytes)?;

        let mut items = Vec::with_capacity(message.items.len());
        for item in message.items {
            let pair = item.pair.ok_or("An item has no pair")?;

            items.push(Item {
                value: item.value,
                first: pair.first,
                second: pair.second,
            });
        }

        Ok(List { items })
    }
}

impl Workload<SampleShape> for Prost {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Sample) -> Result<Vec<u8>, Error> {
        let message = messages::Sample {
            key: input.key,
            a: input.a,
            b: input.b,
        };

        Ok(message.encode_to_vec())
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Sample, Error> {
        let message = messages::Sample::decode(bytes)?;

        Ok(Sample {
            key: message.key,
            a: message.a,
            b: message.b,
        })
    }
}

const fn item_message(item: &Item) -> messages::Item {
    let pair = messages::Pair {
        first: item.first,
        second: item.second,
    };

    messages::Item {
        value: item.value,
        pair: Some(pair),
    }
}
