package io.github.snovader.tafla.benchmarks.jmh;

import io.github.snovader.tafla.benchmarks.*;
import io.github.snovader.tafla.benchmarks.codecs.*;
import org.openjdk.jmh.annotations.*;

/** An array of 100 structs, each of three float32s, which SBE encodes too */
@State(Scope.Benchmark)
public class StructsBenchmark {
    @Param({"tafla", "tafla-gen", "flatbuffers", "protobuf", "msgpack", "sbe"})
    public String codec;

    private ListInput input;
    private Workload<ListInput> workload;
    private byte[] bytes;

    @Setup
    public void setUp() {
        input = Workloads.listInput(Workloads.LIST_ITEMS);
        workload = Codecs.structs(codec);
        bytes = Codecs.serializeChecked(workload, input);
    }

    @Benchmark
    public byte[] serialize() {
        return workload.serialize(input);
    }

    @Benchmark
    public ListInput deserialize() {
        return workload.deserialize(bytes);
    }
}
