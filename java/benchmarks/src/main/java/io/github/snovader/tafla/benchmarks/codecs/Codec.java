package io.github.snovader.tafla.benchmarks.codecs;

import io.github.snovader.tafla.benchmarks.*;

/** One library's encoding of the workloads */
public record Codec(
    String name,
    Workload<RecordInput> record,
    DocumentWorkload document,
    Workload<ListInput> list,
    Workload<SampleInput> sample
) {
    /** {@return the library's name}, which names its codec in test reports */
    @Override
    public String toString() {
        return name;
    }
}
