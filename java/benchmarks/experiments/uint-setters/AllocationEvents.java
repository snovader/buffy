package experiment.diagnostics;

import java.nio.file.Path;
import java.util.*;
import jdk.jfr.consumer.RecordingFile;

/** Streams allocation events without expanding the recording into a large JSON file. */
public final class AllocationEvents {
    private record Allocation(String type, long bytes, String stack) {}

    public static void main(String[] args) throws Exception {
        var counts = new LinkedHashMap<Allocation, Long>();
        try (var recording = new RecordingFile(Path.of(args[0]))) {
            while (recording.hasMoreEvents()) {
                var event = recording.readEvent();
                var name = event.getEventType().getName();
                if (!name.equals("jdk.ObjectAllocationOutsideTLAB")
                        && !name.equals("jdk.ObjectAllocationInNewTLAB")) {
                    continue;
                }
                var trace = event.getStackTrace();
                if (trace == null) {
                    continue;
                }
                var frames = trace.getFrames().stream().map(frame -> {
                    var method = frame.getMethod();
                    return method.getType().getName() + "." + method.getName();
                }).toList();
                if (frames.stream().noneMatch(frame -> frame.endsWith(".tableWrite_" + args[1] + "_32"))) {
                    continue;
                }
                var allocation = new Allocation(event.getClass("objectClass").getName(),
                    event.getLong("allocationSize"), String.join(" -> ", frames));
                counts.merge(allocation, 1L, Long::sum);
            }
        }
        for (var entry : counts.entrySet()) {
            var allocation = entry.getKey();
            System.out.printf("%s\t%d\t%d\t%s%n", allocation.type(), allocation.bytes(),
                entry.getValue(), allocation.stack());
        }
    }
}
