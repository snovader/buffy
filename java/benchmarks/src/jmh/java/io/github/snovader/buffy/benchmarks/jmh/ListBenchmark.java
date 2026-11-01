package io.github.snovader.buffy.benchmarks.jmh;

import io.github.snovader.buffy.benchmarks.*;
import io.github.snovader.buffy.benchmarks.codecs.*;
import org.openjdk.jmh.annotations.*;

/** A vector of 100 nested tables, each a float32 and a nested table of two float32s */
@State(Scope.Benchmark)
public class ListBenchmark {
    @Param({"buffy", "flatbuffers", "protobuf", "msgpack"})
    public String codec;

    private ListInput input;
    private Workload<ListInput> workload;
    private byte[] bytes;

    @Setup
    public void setUp() {
        input = Workloads.listInput(Workloads.LIST_ITEMS);
        workload = Codecs.named(codec).list();
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
