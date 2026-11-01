use crate::codec::{Codec, Error, Workload};
use crate::workloads::{Sample, SampleShape};

const SAMPLE_SIZE: usize = 12;

/// The standard library's `to_le_bytes()` and `from_le_bytes()`, for the
/// fixed-size record only: the obvious Rust code for fields at fixed offsets,
/// which buffy's README recommends for them.
///
/// It writes the record as buffy's codec does, as buffy-gen generates a
/// struct's writer, so the two serialize the same way. Reading takes the
/// record's bytes as an array, and fails only when there are fewer.
#[derive(Clone, Copy, Debug)]
pub struct LeBytes;

impl Codec for LeBytes {
    const NAME: &'static str = "to_le_bytes";
}

impl Workload<SampleShape> for LeBytes {
    type Bytes = [u8; SAMPLE_SIZE];

    fn serialize(self, input: &Sample) -> Result<[u8; SAMPLE_SIZE], Error> {
        let mut bytes = [0; SAMPLE_SIZE];
        bytes[0..4].copy_from_slice(&input.key.to_le_bytes());
        bytes[4..8].copy_from_slice(&input.a.to_le_bytes());
        bytes[8..12].copy_from_slice(&input.b.to_le_bytes());

        Ok(bytes)
    }

    fn deserialize(self, bytes: &[u8]) -> Result<Sample, Error> {
        let Some(&[k0, k1, k2, k3, a0, a1, a2, a3, b0, b1, b2, b3]) =
            bytes.first_chunk::<SAMPLE_SIZE>()
        else {
            return Err(format!("A sample needs {SAMPLE_SIZE} bytes, not {}", bytes.len()).into());
        };

        Ok(Sample {
            key: u32::from_le_bytes([k0, k1, k2, k3]),
            a: f32::from_le_bytes([a0, a1, a2, a3]),
            b: f32::from_le_bytes([b0, b1, b2, b3]),
        })
    }
}
