/**
 * Each library's encoding of the workloads. A codec's workloads may reuse buffers or flyweights
 * between calls, as their libraries intend, so a codec is for one thread.
 */
@NullMarked
package io.github.snovader.tafla.benchmarks.codecs;

import org.jspecify.annotations.*;
