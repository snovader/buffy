//! The compiler's WebAssembly interface: plain exports with no imports, so
//! any host runs the same module without WASI or bindings.
//!
//! A host allocates a request with `buffy_alloc`, writes it and passes it to
//! `buffy_compile`, which frees it and returns a response. The host reads the
//! response's length from its first four bytes and frees it with `buffy_free`.
//!
//! Every string is a little-endian `u32` byte length and UTF-8 bytes:
//!
//! - request: target, file name, source, runtime, package; an empty runtime
//!   or package takes the target's default, and a package that isn't a Java
//!   package is an error
//! - response: its `u32` length, a `u32` status, then for status 0 a `u32`
//!   file count and each file's path and contents, and for status 1 the
//!   rendered errors

use crate::{Diagnostic, JavaPackage, Options, Target, compile};

/// Allocates exactly `length` bytes for a request
#[unsafe(no_mangle)]
pub extern "C" fn buffy_alloc(length: usize) -> *mut u8 {
    Box::into_raw(vec![0_u8; length].into_boxed_slice()).cast()
}

/// # Safety
///
/// `pointer` must come from `buffy_alloc` or `buffy_compile` with `length`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn buffy_free(pointer: *mut u8, length: usize) {
    drop(unsafe { take_back(pointer, length) });
}

/// # Safety
///
/// `pointer` must come from `buffy_alloc` with `length`, and hold the request.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn buffy_compile(pointer: *mut u8, length: usize) -> *mut u8 {
    let request = unsafe { take_back(pointer, length) };
    let response = respond(&request);

    Box::into_raw(response.into_boxed_slice()).cast()
}

/// Takes back the bytes that `buffy_alloc` or `buffy_compile` handed out.
/// They're boxed slices, which are deallocated at exactly their length.
///
/// # Safety
///
/// As for `buffy_free`.
unsafe fn take_back(pointer: *mut u8, length: usize) -> Box<[u8]> {
    unsafe { Box::from_raw(std::ptr::slice_from_raw_parts_mut(pointer, length)) }
}

fn respond(request: &[u8]) -> Vec<u8> {
    let mut reader = Reader { bytes: request };
    let mut response = vec![0; 4];

    match handle(&mut reader) {
        Ok(files) => {
            response.extend_from_slice(&0_u32.to_le_bytes());
            push_length(&mut response, files.len());
            for file in files {
                push_string(&mut response, &file.path);
                push_string(&mut response, &file.contents);
            }
        }
        Err(errors) => {
            response.extend_from_slice(&1_u32.to_le_bytes());
            push_string(&mut response, &errors);
        }
    }

    let length = u32::try_from(response.len()).unwrap_or(u32::MAX);
    response[..4].copy_from_slice(&length.to_le_bytes());
    response
}

fn handle(reader: &mut Reader) -> Result<Vec<crate::GeneratedFile>, String> {
    let malformed = || "malformed request\n".to_owned();
    let target_name = reader.string().ok_or_else(malformed)?;
    let file_name = reader.string().ok_or_else(malformed)?;
    let source = reader.string().ok_or_else(malformed)?;
    let runtime = reader.string().ok_or_else(malformed)?;
    let package = reader.string().ok_or_else(malformed)?;

    let target = Target::from_name(target_name).ok_or_else(|| {
        let targets: Vec<_> = Target::names().collect();
        format!(
            "unknown target `{target_name}`: this build generates {}\n",
            targets.join(", ")
        )
    })?;
    let package = (!package.is_empty())
        .then(|| package.parse::<JavaPackage>())
        .transpose()
        .map_err(|error| format!("{error}\n"))?;
    let options = Options {
        runtime: (!runtime.is_empty()).then(|| runtime.to_owned()),
        package,
    };

    compile(file_name, source, target, &options)
        .map_err(|diagnostics| Diagnostic::render_all(&diagnostics, file_name, source))
}

struct Reader<'a> {
    bytes: &'a [u8],
}

impl<'a> Reader<'a> {
    fn string(&mut self) -> Option<&'a str> {
        let (length, rest) = self.bytes.split_first_chunk::<4>()?;
        let length = u32::from_le_bytes(*length) as usize;
        let (string, rest) = rest.split_at_checked(length)?;
        self.bytes = rest;
        std::str::from_utf8(string).ok()
    }
}

fn push_length(bytes: &mut Vec<u8>, length: usize) {
    let length = u32::try_from(length).unwrap_or(u32::MAX);
    bytes.extend_from_slice(&length.to_le_bytes());
}

fn push_string(bytes: &mut Vec<u8>, string: &str) {
    push_length(bytes, string.len());
    bytes.extend_from_slice(string.as_bytes());
}
