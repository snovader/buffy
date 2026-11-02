//! Checks that every codec reads back exactly what it wrote of every workload,
//! so that the benchmarks can't time a broken codec

use tafla_benchmarks::check;
use tafla_benchmarks::codec::{Error, Workload};
use tafla_benchmarks::codecs::{LeBytes, Tafla, TaflaGen};
use tafla_benchmarks::for_each_codec;
use tafla_benchmarks::workloads::{
    self, DOCUMENT_ROWS, DocumentShape, LIST_ITEMS, ListShape, RECORD, RecordShape, SAMPLE,
    SampleShape, Shape, StructsShape,
};

#[test]
fn every_codec_reads_back_the_record() -> Result<(), Error> {
    for_each_codec!(|codec| check::round_trip::<RecordShape, _>(codec, &RECORD)?);
    Ok(())
}

#[test]
fn every_codec_reads_back_the_documents() -> Result<(), Error> {
    for rows in DOCUMENT_ROWS {
        let document = workloads::document(rows);
        for_each_codec!(|codec| check::round_trip::<DocumentShape, _>(codec, &document)?);
    }

    Ok(())
}

#[test]
fn every_codec_sums_a_column_of_the_documents() -> Result<(), Error> {
    for rows in DOCUMENT_ROWS {
        let document = workloads::document(rows);
        for_each_codec!(|codec| check::sum_a(codec, &document)?);
    }

    Ok(())
}

#[test]
fn every_codec_reads_back_the_vector_of_nested_tables() -> Result<(), Error> {
    let list = workloads::list(LIST_ITEMS);
    for_each_codec!(|codec| check::round_trip::<ListShape, _>(codec, &list)?);

    Ok(())
}

#[test]
fn every_codec_reads_back_the_array_of_structs() -> Result<(), Error> {
    let list = workloads::list(LIST_ITEMS);
    for_each_codec!(|codec| check::round_trip::<StructsShape, _>(codec, &list)?);

    Ok(())
}

#[test]
fn every_codec_reads_back_the_fixed_size_record() -> Result<(), Error> {
    for_each_codec!(|codec| check::round_trip::<SampleShape, _>(codec, &SAMPLE)?);
    check::round_trip::<SampleShape, _>(LeBytes, &SAMPLE)
}

#[test]
fn generated_code_writes_the_builders_tables() -> Result<(), Error> {
    let list = workloads::list(LIST_ITEMS);

    assert_same_bytes::<RecordShape>(&RECORD)?;
    for rows in DOCUMENT_ROWS {
        assert_same_bytes::<DocumentShape>(&workloads::document(rows))?;
    }
    assert_same_bytes::<ListShape>(&list)?;
    assert_same_bytes::<StructsShape>(&list)?;
    assert_same_bytes::<SampleShape>(&SAMPLE)
}

fn assert_same_bytes<S: Shape>(input: &S::Value<'_>) -> Result<(), Error>
where
    Tafla: Workload<S>,
    TaflaGen: Workload<S>,
{
    let built = Tafla.serialize(input)?;
    let generated = TaflaGen.serialize(input)?;

    assert_eq!(generated.as_ref(), built.as_ref(), "the {}", S::NAME);
    Ok(())
}
