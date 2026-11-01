package io.github.snovader.buffy.benchmarks.jmh;

import io.github.snovader.buffy.benchmarks.*;
import io.github.snovader.buffy.benchmarks.codecs.*;
import org.openjdk.jmh.annotations.*;

/** The record: two float64s, an int64 and a 16-byte string */
@State(Scope.Benchmark)
public class RecordBenchmark {
    @Param({"buffy", "flatbuffers", "protobuf", "msgpack"})
    public String codec;

    // Fields rather than constants, so that the JIT can't fold the input into the code
    private RecordInput input;
    private Workload<RecordInput> workload;
    private byte[] bytes;

    @Setup
    public void setUp() {
        input = Workloads.RECORD;
        workload = Codecs.named(codec).record();
        bytes = Codecs.serializeChecked(workload, input);
    }

    @Benchmark
    public byte[] serialize() {
        return workload.serialize(input);
    }

    @Benchmark
    public RecordInput deserialize() {
        return workload.deserialize(bytes);
    }
}
