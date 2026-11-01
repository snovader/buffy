//! Each library's encoding of the workloads

mod buffy;
mod flatbuffers;
mod le_bytes;
mod msgpack;
mod protobuf;
mod rkyv;

pub use self::buffy::Buffy;
pub use self::flatbuffers::{Finished, FlatBuffers};
pub use self::le_bytes::LeBytes;
pub use self::msgpack::RmpSerde;
pub use self::protobuf::{Prost, messages};
pub use self::rkyv::Rkyv;

/// Expands `$body` once for each codec that encodes every workload.
///
/// `$codec` is bound to each in the order that results list them. It's a
/// macro rather than a loop, since each codec is a type of its own, which its
/// benchmarks are compiled for.
///
/// [`LeBytes`] encodes only the fixed-size record, so callers add it there.
#[macro_export]
macro_rules! for_each_codec {
    (|$codec:ident| $body:expr) => {{
        {
            let $codec = $crate::codecs::Buffy;
            $body;
        }
        {
            let $codec = $crate::codecs::FlatBuffers;
            $body;
        }
        {
            let $codec = $crate::codecs::Prost;
            $body;
        }
        {
            let $codec = $crate::codecs::RmpSerde;
            $body;
        }
        {
            let $codec = $crate::codecs::Rkyv;
            $body;
        }
    }};
}
