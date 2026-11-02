// The codec uses a part of the generated code. tafla-gen's tests check its
// code with clippy's pedantic lints, and this crate's nursery lints suggest
// otherwise, such as `map_or()` for a `match` of an `Option`.
#[allow(dead_code, clippy::nursery)]
#[path = "tafla_gen/workloads.rs"]
mod generated;

use std::borrow::Cow;

use self::generated::{
    ColumnsFields, DocumentFields, ItemFields, ItemList, ItemListFields, PairFields, RecordFields,
    Triple, Triples, TriplesFields,
};
use crate::codec::{Codec, DocumentWorkload, Error, Workload};
use crate::workloads::{
    Document, DocumentShape, Item, List, ListShape, Record, RecordShape, Sample, SampleShape,
    StructsShape, sum_column,
};

/// The code that tafla-gen generates from `workloads.tafla`.
///
/// It writes each message's table in one call with a `TableSizer` and a
/// `TableWriter`, rather than nesting `TableBuilder`s, and reads it with the
/// generated getters, which borrow strings and arrays as the `Tafla` codec
/// does.
///
/// The generated code writes from structs of each table's fields, which
/// borrow strings and arrays, and from slices of the items' fields and of the
/// structs, which `serialize()` collects from the workloads' items in each
/// call, as the prost codec collects its messages. The generated reader of
/// a vector of tables collects the items' readers, which `deserialize()` then
/// reads into the workload's items.
#[derive(Clone, Copy, Debug)]
pub struct TaflaGen;

impl Codec for TaflaGen {
    const NAME: &'static str = "tafla-gen";
}

impl Workload<RecordShape> for TaflaGen {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Record<'_>) -> Result<Vec<u8>, Error> {
        let fields = RecordFields {
            x: input.x,
            y: input.y,
            id: input.id,
            name: input.name.as_ref(),
        };

        Ok(fields.serialize()?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Record<'_>, Error> {
        let record = generated::Record::new(bytes);

        Ok(Record {
            x: record.x()?,
            y: record.y()?,
            id: record.id()?,
            name: Cow::Borrowed(record.name()?),
        })
    }
}

impl Workload<DocumentShape> for TaflaGen {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &Document<'_>) -> Result<Vec<u8>, Error> {
        let columns = ColumnsFields {
            base: input.base,
            keys: input.keys.as_ref(),
            a: input.a.as_ref(),
            b: input.b.as_ref(),
        };
        let fields = DocumentFields {
            x: input.x,
            y: input.y,
            name: input.name.as_ref(),
            columns,
        };

        Ok(fields.serialize()?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Document<'_>, Error> {
        let document = generated::Document::new(bytes)?;
        let columns = document.columns()?;

        Ok(Document {
            x: document.x()?,
            y: document.y()?,
            name: Cow::Borrowed(document.name()?),
            base: columns.base()?,
            keys: columns.keys()?.to_cow(),
            a: columns.a()?.to_cow(),
            b: columns.b()?.to_cow(),
        })
    }
}

impl DocumentWorkload for TaflaGen {
    fn sum_a(self, bytes: &[u8]) -> Result<f64, Error> {
        let a = generated::Document::new(bytes)?.columns()?.a()?;

        Ok(sum_column(a.iter()))
    }
}

impl Workload<ListShape> for TaflaGen {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &List) -> Result<Vec<u8>, Error> {
        let items: Vec<ItemFields> = input.items.iter().map(item_fields).collect();

        Ok(ItemListFields { items: &items }.serialize()?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<List, Error> {
        let tables = ItemList::new(bytes).items()?;

        let mut items = Vec::with_capacity(tables.len());
        for item in &tables {
            let pair = item.pair()?;
            items.push(Item {
                value: item.value()?,
                first: pair.first()?,
                second: pair.second()?,
            });
        }

        Ok(List { items })
    }
}

impl Workload<StructsShape> for TaflaGen {
    type Bytes = Vec<u8>;

    fn serialize(self, input: &List) -> Result<Vec<u8>, Error> {
        let triples: Vec<Triple> = input.items.iter().map(triple).collect();

        Ok(TriplesFields { items: &triples }.serialize()?)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<List, Error> {
        let items = Triples::new(bytes).items()?.map(item_of_triple).collect();

        Ok(List { items })
    }
}

impl Workload<SampleShape> for TaflaGen {
    type Bytes = [u8; generated::Sample::SIZE];

    fn serialize(self, input: &Sample) -> Result<[u8; generated::Sample::SIZE], Error> {
        let sample = generated::Sample {
            key: input.key,
            a: input.a,
            b: input.b,
        };

        Ok(sample.to_bytes())
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Sample, Error> {
        let sample = generated::Sample::read(bytes)?;

        Ok(Sample {
            key: sample.key,
            a: sample.a,
            b: sample.b,
        })
    }
}

const fn item_fields(item: &Item) -> ItemFields {
    ItemFields {
        value: item.value,
        pair: PairFields {
            first: item.first,
            second: item.second,
        },
    }
}

const fn triple(item: &Item) -> Triple {
    Triple {
        value: item.value,
        first: item.first,
        second: item.second,
    }
}

const fn item_of_triple(triple: Triple) -> Item {
    Item {
        value: triple.value,
        first: triple.first,
        second: triple.second,
    }
}
