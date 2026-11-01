package io.github.snovader.buffy.benchmarks.codecs;

import io.github.snovader.buffy.benchmarks.*;

import java.util.*;

/** The libraries that the comparison runs, with buffy first as the reference */
public final class Codecs {
    /** The name of SBE's encoding of the fixed-size record */
    public static final String SBE = "sbe";

    private Codecs() {}

    /** {@return a new codec of each library that encodes every workload} */
    public static List<Codec> all() {
        return List.of(
            BuffyCodec.create(),
            FlatBuffersCodec.create(),
            ProtobufCodec.create(),
            MessagePackCodec.create()
        );
    }

    /**
     * {@return a new codec of the library}
     *
     * @throws IllegalArgumentException if no codec has that name
     */
    public static Codec named(String name) {
        return all().stream()
            .filter(codec -> codec.name().equals(name))
            .findFirst()
            .orElseThrow(() -> new IllegalArgumentException("No codec is named " + name));
    }

    /**
     * {@return new encodings of the fixed-size record by name: every codec's, and SBE's, which
     * encodes only fixed-size messages}
     */
    public static SequencedMap<String, Workload<SampleInput>> samples() {
        var samples = new LinkedHashMap<String, Workload<SampleInput>>();
        for (var codec : all()) {
            samples.put(codec.name(), codec.sample());
        }
        samples.put(SBE, new SbeSampleWorkload());

        return samples;
    }

    /**
     * {@return a new encoding of the fixed-size record by a codec or by SBE}
     *
     * @throws IllegalArgumentException if no codec has that name
     */
    public static Workload<SampleInput> sample(String name) {
        var sample = samples().get(name);
        if (sample == null) {
            throw new IllegalArgumentException("No codec is named " + name);
        }

        return sample;
    }

    /**
     * Serializes the input after checking that the workload reads back exactly what it wrote, so
     * that a broken codec isn't timed.
     *
     * @throws IllegalStateException if it doesn't
     */
    public static <T> byte[] serializeChecked(Workload<T> workload, T input) {
        var bytes = workload.serialize(input);
        var output = workload.deserialize(bytes);

        if (!output.equals(input)) {
            throw new IllegalStateException(
                String.format("%s read back %s as %s", workload.getClass().getName(), input, output)
            );
        }

        return bytes;
    }
}
