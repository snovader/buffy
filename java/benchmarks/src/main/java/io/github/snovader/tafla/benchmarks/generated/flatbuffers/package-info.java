/**
 * FlatBuffers' accessors and builders of the workloads' tables, written by hand in the shape that
 * {@code flatc --java} generates from {@code src/main/flatbuffers/workloads.fbs}, with only the
 * accessors and builders that the benchmarks use. Like flatc's code, it isn't null-marked, since
 * accessors of strings and tables return {@code null} for a field that the table doesn't have, and
 * Error Prone skips it, as it skips protoc's and sbe-tool's generated code.
 */
package io.github.snovader.tafla.benchmarks.generated.flatbuffers;
