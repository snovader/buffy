package io.github.snovader.buffy.benchmarks.jmh;

import io.github.snovader.buffy.benchmarks.*;
import io.github.snovader.buffy.benchmarks.codecs.*;
import org.openjdk.jmh.annotations.*;

/** The fixed-size record: a uint32 and two float32s, which SBE encodes too */
@State(Scope.Benchmark)
public class SampleBenchmark {
    @Param({"buffy", "flatbuffers", "protobuf", "msgpack", "sbe"})
    public String codec;

    private SampleInput input;
    private Workload<SampleInput> workload;
    private byte[] bytes;

    @Setup
    public void setUp() {
        input = Workloads.SAMPLE;
        workload = Codecs.sample(codec);
        bytes = Codecs.serializeChecked(workload, input);
    }

    @Benchmark
    public byte[] serialize() {
        return workload.serialize(input);
    }

    @Benchmark
    public SampleInput deserialize() {
        return workload.deserialize(bytes);
    }
}
