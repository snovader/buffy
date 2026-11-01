package experiment.diagnostics;

import java.lang.instrument.Instrumentation;

/** Used only in separate diagnostic JVMs, never in the timing comparisons. */
public final class AllocationProbe {
    private AllocationProbe() {}

    public static void premain(String variant, Instrumentation instrumentation) throws Exception {
        var type = Class.forName("experiment." + variant + ".ByteBuffer");
        var buffer = type.getMethod("allocate", int.class).invoke(null, 104);
        var viewField = type.getDeclaredField("view");
        viewField.setAccessible(true);
        var view = viewField.get(buffer);
        var bytes = type.getMethod("bytes").invoke(buffer);

        System.out.printf("SIZE %s %d%n", type.getName(), instrumentation.getObjectSize(buffer));
        System.out.printf("SIZE %s %d%n", view.getClass().getName(), instrumentation.getObjectSize(view));
        System.out.printf("SIZE byte[104] %d%n", instrumentation.getObjectSize(bytes));
    }
}
