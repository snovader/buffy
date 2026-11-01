//! Times tafla's runtime and other Rust crates on the TypeScript comparison's
//! workloads, as README.md describes. `cargo run --bin report` then prints
//! each benchmark's fastest sample.

use std::hint::black_box;
use std::time::Duration;

use criterion::measurement::WallTime;
use criterion::{BenchmarkGroup, Criterion, criterion_group, criterion_main};
use tafla_benchmarks::check;
use tafla_benchmarks::codec::{DocumentWorkload, Error, Workload};
use tafla_benchmarks::codecs::LeBytes;
use tafla_benchmarks::for_each_codec;
use tafla_benchmarks::workloads::{
    self, DOCUMENT_ROWS, Document, DocumentShape, LIST_ITEMS, ListShape, RECORD, RecordShape,
    SAMPLE, SampleShape, Shape,
};

type Group<'a> = BenchmarkGroup<'a, WallTime>;

criterion_group! {
    name = comparison;
    // 100 samples, which grow linearly from about a millisecond, so that the
    // fastest isn't one lucky call. Command-line options override these.
    config = Criterion::default()
        .warm_up_time(Duration::from_secs(2))
        .measurement_time(Duration::from_secs(4));
    targets = record, documents, list, sample
}

criterion_main!(comparison);

fn record(criterion: &mut Criterion) {
    group(criterion, "record: serialize", |group| {
        for_each_codec!(|codec| serialize::<RecordShape, _>(group, codec, &RECORD));
    });
    group(criterion, "record: deserialize", |group| {
        for_each_codec!(|codec| deserialize::<RecordShape, _>(group, codec, &RECORD));
    });
}

fn documents(criterion: &mut Criterion) {
    for rows in DOCUMENT_ROWS {
        let document = workloads::document(rows);
        let name = format!("document of {} rows", thousands(rows));

        group(criterion, &format!("{name}: serialize"), |group| {
            for_each_codec!(|codec| serialize::<DocumentShape, _>(group, codec, &document));
        });
        group(criterion, &format!("{name}: deserialize"), |group| {
            for_each_codec!(|codec| deserialize::<DocumentShape, _>(group, codec, &document));
        });
        group(
            criterion,
            &format!("{name}: sum a column per element"),
            |group| {
                for_each_codec!(|codec| sum_a(group, codec, &document));
            },
        );
    }
}

fn list(criterion: &mut Criterion) {
    let list = workloads::list(LIST_ITEMS);
    let name = format!("vector of {LIST_ITEMS} nested tables");

    group(criterion, &format!("{name}: serialize"), |group| {
        for_each_codec!(|codec| serialize::<ListShape, _>(group, codec, &list));
    });
    group(criterion, &format!("{name}: deserialize"), |group| {
        for_each_codec!(|codec| deserialize::<ListShape, _>(group, codec, &list));
    });
}

fn sample(criterion: &mut Criterion) {
    group(criterion, "fixed-size record: serialize", |group| {
        for_each_codec!(|codec| serialize::<SampleShape, _>(group, codec, &SAMPLE));
        serialize::<SampleShape, _>(group, LeBytes, &SAMPLE);
    });
    group(criterion, "fixed-size record: deserialize", |group| {
        for_each_codec!(|codec| deserialize::<SampleShape, _>(group, codec, &SAMPLE));
        deserialize::<SampleShape, _>(group, LeBytes, &SAMPLE);
    });
}

/// Runs a group of benchmarks, which `add` adds a codec at a time, so that
/// the codecs run one after another
fn group(criterion: &mut Criterion, name: &str, add: impl FnOnce(&mut Group<'_>)) {
    let mut group = criterion.benchmark_group(name);
    add(&mut group);
    group.finish();
}

/// Times writing `input`. The bytes are dropped within the timed loop, as a
/// caller frees them.
fn serialize<S: Shape, C: Workload<S>>(group: &mut Group<'_>, codec: C, input: &S::Value<'_>) {
    checked(check::round_trip::<S, C>(codec, input));

    group.bench_function(C::NAME, |bencher| {
        bencher.iter(|| codec.serialize(black_box(input)));
    });
}

/// Times reading the bytes that the codec wrote of `input`
fn deserialize<S: Shape, C: Workload<S>>(group: &mut Group<'_>, codec: C, input: &S::Value<'_>) {
    checked(check::round_trip::<S, C>(codec, input));
    let bytes = checked(codec.serialize(input));

    group.bench_function(C::NAME, |bencher| {
        bencher.iter(|| codec.deserialize(black_box(bytes.as_ref())));
    });
}

/// Times summing column `a` of the bytes that the codec wrote of `input`
fn sum_a<C: DocumentWorkload>(group: &mut Group<'_>, codec: C, input: &Document<'_>) {
    checked(check::sum_a(codec, input));
    let bytes = checked(codec.serialize(input));

    group.bench_function(C::NAME, |bencher| {
        bencher.iter(|| codec.sum_a(black_box(bytes.as_ref())));
    });
}

/// Stops the benchmarks before timing a codec that failed its check
fn checked<T>(result: Result<T, Error>) -> T {
    result.unwrap_or_else(|error| panic!("{error}"))
}

/// Formats `100000` as `100,000`, as the other languages name the groups
fn thousands(value: usize) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);

    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            formatted.push(',');
        }
        formatted.push(digit);
    }

    formatted
}
