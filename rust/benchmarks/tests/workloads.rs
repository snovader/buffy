//! Checks that the generator makes the TypeScript comparison's values, which
//! these were printed from, so that the languages' benchmarks time the same
//! work

use buffy_benchmarks::workloads::{self, Document, Item, LIST_ITEMS};

#[test]
fn generates_the_typescript_document_of_100_rows() {
    let document = workloads::document(100);

    assert_eq!(document.keys[0], 14_727_226);
    assert_eq!(document.keys[99], 10_335_322);
    assert_eq!(sum(&document.keys), 825_833_448);
    assert_eq!(document.a[0].to_bits(), 0xc221_9b29);
    assert_eq!(document.b[99].to_bits(), 0xc163_647e);
    assert_eq!(hash_columns(&document), 1_493_961_445);
}

#[test]
fn generates_the_typescript_document_of_100000_rows() {
    let document = workloads::document(100_000);

    assert_eq!(document.keys[0], 13_320_945);
    assert_eq!(document.keys[99_999], 11_621_954);
    assert_eq!(sum(&document.keys), 839_050_642_924);
    assert_eq!(document.a[0].to_bits(), 0xc1d4_d7f6);
    assert_eq!(document.b[99_999].to_bits(), 0x3e8c_91f5);
    assert_eq!(hash_columns(&document), 1_138_446_770);
}

#[test]
fn generates_the_typescript_vector_of_nested_tables() {
    let items = workloads::list(LIST_ITEMS).items;

    assert_eq!(items.len(), 100);
    assert_eq!(bits(items[0]), [0x41d2_acb6, 0xbfce_db15, 0x41e3_2e23]);
    assert_eq!(bits(items[1]), [0x41c1_2b3a, 0x3ea9_43c1, 0x422e_ae51]);
    assert_eq!(bits(items[99]), [0x4193_d915, 0x3f97_9e05, 0x418f_26e1]);
}

fn sum(keys: &[u32]) -> u64 {
    keys.iter().copied().map(u64::from).sum()
}

/// Hashes the float columns' bits, row by row, as the TypeScript script that
/// printed these did with 32-bit integers
fn hash_columns(document: &Document<'_>) -> u32 {
    document
        .a
        .iter()
        .zip(document.b.iter())
        .fold(0_u32, |hash, (a, b)| {
            let hash = hash.wrapping_mul(31).wrapping_add(a.to_bits());
            hash.wrapping_mul(31).wrapping_add(b.to_bits())
        })
}

const fn bits(item: Item) -> [u32; 3] {
    [
        item.value.to_bits(),
        item.first.to_bits(),
        item.second.to_bits(),
    ]
}
