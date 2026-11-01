package io.github.snovader.tafla.benchmarks.codecs;

import io.github.snovader.tafla.benchmarks.*;

public interface DocumentWorkload extends Workload<DocumentInput> {
    /** Sums column {@code a} through the library's own per-element accessor */
    double sumA(byte[] bytes);
}
