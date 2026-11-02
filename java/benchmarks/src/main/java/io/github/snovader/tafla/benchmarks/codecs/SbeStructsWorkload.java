package io.github.snovader.tafla.benchmarks.codecs;

import io.github.snovader.tafla.benchmarks.*;
import io.github.snovader.tafla.benchmarks.generated.sbe.*;
import io.github.snovader.tafla.benchmarks.generated.sbe.TriplesEncoder.ItemsEncoder;
import org.agrona.concurrent.*;

import java.util.Arrays;

/**
 * Simple Binary Encoding's generated encoder and decoder of the array of structs: a message whose
 * repeating group holds an entry of the three floats for each item, at fixed offsets, after the
 * group's 4-byte header of its entries' length and count. As for the fixed-size record, the
 * message starts with an 8-byte header, and the flyweights are reused for every message.
 */
public final class SbeStructsWorkload implements Workload<ListInput> {
    private static final int HEADER_LENGTH =
        MessageHeaderEncoder.ENCODED_LENGTH + TriplesEncoder.BLOCK_LENGTH + ItemsEncoder.HEADER_SIZE;

    private final UnsafeBuffer buffer = new UnsafeBuffer();
    private final MessageHeaderEncoder headerEncoder = new MessageHeaderEncoder();
    private final TriplesEncoder encoder = new TriplesEncoder();
    private final MessageHeaderDecoder headerDecoder = new MessageHeaderDecoder();
    private final TriplesDecoder decoder = new TriplesDecoder();

    @Override
    public byte[] serialize(ListInput input) {
        var items = input.items();
        var bytes = new byte[HEADER_LENGTH + items.size() * ItemsEncoder.sbeBlockLength()];
        buffer.wrap(bytes);

        var group = encoder.wrapAndApplyHeader(buffer, 0, headerEncoder).itemsCount(items.size());
        for (var item : items) {
            group.next().value(item.value()).first(item.first()).second(item.second());
        }

        return bytes;
    }

    /** @throws IllegalStateException if the message has another template */
    @Override
    public ListInput deserialize(byte[] bytes) {
        buffer.wrap(bytes);
        var group = decoder.wrapAndApplyHeader(buffer, 0, headerDecoder).items();

        var items = new ItemInput[group.count()];
        for (int i = 0; i < items.length; i++) {
            group.next();
            items[i] = new ItemInput(group.value(), group.first(), group.second());
        }

        return new ListInput(Arrays.asList(items));
    }
}
