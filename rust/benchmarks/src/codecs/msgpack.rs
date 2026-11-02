use crate::codec::{Codec, DocumentWorkload, Error, Workload};
use crate::workloads::{
    Document, DocumentShape, List, ListShape, Record, RecordShape, Sample, SampleShape,
    StructsShape, sum_column,
};

/// rmp-serde, which packs the workloads' own types through their serde derives.
///
/// The msgpack format is schemaless: a struct packs as a map with its field
/// names, as `to_vec_named()` writes it and msgpackr packs an object, and a
/// document packs as one map. Columns pack as arrays of float32s and uint32s,
/// element by element, since msgpack has no typed arrays. Reading borrows
/// strings from the bytes and copies columns into `Vec`s.
///
/// Structs have a fixed shape, so an array of them packs with `to_vec()`,
/// which packs a struct as an array of its fields in order, without their
/// names: each item, and the table that holds them.
#[derive(Clone, Copy, Debug)]
pub struct RmpSerde;

impl Codec for RmpSerde {
    const NAME: &'static str = "rmp-serde";
}

impl Workload<RecordShape> for RmpSerde {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Record<'_>) -> Result<Vec<u8>, Error> {
        Ok(rmp_serde::to_vec_named(input)?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Record<'_>, Error> {
        Ok(rmp_serde::from_slice(bytes)?)
    }
}

impl Workload<DocumentShape> for RmpSerde {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Document<'_>) -> Result<Vec<u8>, Error> {
        Ok(rmp_serde::to_vec_named(input)?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Document<'_>, Error> {
        Ok(rmp_serde::from_slice(bytes)?)
    }
}

impl DocumentWorkload for RmpSerde {
    fn sum_a(self, bytes: &[u8]) -> Result<f64, Error> {
        let document: Document<'_> = rmp_serde::from_slice(bytes)?;
        Ok(sum_column(document.a.iter().copied()))
    }
}

impl Workload<ListShape> for RmpSerde {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &List) -> Result<Vec<u8>, Error> {
        Ok(rmp_serde::to_vec_named(input)?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<List, Error> {
        Ok(rmp_serde::from_slice(bytes)?)
    }
}

impl Workload<StructsShape> for RmpSerde {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &List) -> Result<Vec<u8>, Error> {
        Ok(rmp_serde::to_vec(input)?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<List, Error> {
        Ok(rmp_serde::from_slice(bytes)?)
    }
}

impl Workload<SampleShape> for RmpSerde {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Sample) -> Result<Vec<u8>, Error> {
        Ok(rmp_serde::to_vec_named(input)?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Sample, Error> {
        Ok(rmp_serde::from_slice(bytes)?)
    }
}
