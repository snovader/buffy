package io.github.snovader.buffy.benchmarks.codecs;

import io.github.snovader.buffy.benchmarks.*;
import io.github.snovader.buffy.benchmarks.generated.sbe.*;
import org.agrona.concurrent.*;

/**
 * Simple Binary Encoding's generated encoder and decoder of the fixed-size record, with the 8-byte
 * message header that a decoder checks the message's template by. SBE encodes fields at fixed
 * offsets, as buffy's fixed-size record does, through flyweights that are reused for every message,
 * as SBE intends.
 */
public final class SbeSampleWorkload implements Workload<SampleInput> {
    private static final int LENGTH = MessageHeaderEncoder.ENCODED_LENGTH + SampleEncoder.BLOCK_LENGTH;

    private final UnsafeBuffer buffer = new UnsafeBuffer();
    private final MessageHeaderEncoder headerEncoder = new MessageHeaderEncoder();
    private final SampleEncoder encoder = new SampleEncoder();
    private final MessageHeaderDecoder headerDecoder = new MessageHeaderDecoder();
    private final SampleDecoder decoder = new SampleDecoder();

    @Override
    public byte[] serialize(SampleInput input) {
        var bytes = new byte[LENGTH];
        buffer.wrap(bytes);

        encoder.wrapAndApplyHeader(buffer, 0, headerEncoder)
            .key(input.key())
            .a(input.a())
            .b(input.b());

        return bytes;
    }

    /** @throws IllegalStateException if the message has another template */
    @Override
    public SampleInput deserialize(byte[] bytes) {
        buffer.wrap(bytes);
        decoder.wrapAndApplyHeader(buffer, 0, headerDecoder);

        return new SampleInput(decoder.key(), decoder.a(), decoder.b());
    }
}
