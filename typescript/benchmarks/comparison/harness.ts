export interface Variant {
    name: string;
    run: () => unknown;
}

export interface Measurement {
    name: string;
    /** The fastest call over all rounds, in microseconds */
    microseconds: number;
}

export interface MeasureOptions {
    rounds: number;
    warmupRounds: number;
    /** The time that one round of one variant should take, in milliseconds */
    sampleTime: number;
}

type Timer = (run: () => unknown, repetitions: number) => number;

let timerId = 0;

/**
 * Times the variants interleaved: every round runs each of them once, in
 * alternating order, and the fastest round counts. Load from other processes
 * then slows every variant alike, and only for some rounds. Each variant gets
 * its own timer loop, compiled from a unique source, so that its call site
 * and type feedback aren't shared with the others.
 */
export function measure(
    variants: Variant[],
    { rounds, warmupRounds, sampleTime }: MeasureOptions
): Measurement[] {
    const timers = variants.map(createTimer);
    const repetitions = variants.map(variant => calibrate(variant, sampleTime));
    const best = variants.map(() => Number.POSITIVE_INFINITY);

    for (let round = -warmupRounds; round < rounds; round++) {
        for (let j = 0; j < variants.length; j++) {
            const i = round % 2 === 0 ? j : variants.length - 1 - j;
            const time = timers[i](variants[i].run, repetitions[i]);

            if (round >= 0) {
                best[i] = Math.min(best[i], time);
            }
        }
    }

    return variants.map(({ name }, i) => ({ name, microseconds: best[i] }));
}

function createTimer(): Timer {
    // Identical sources would share feedback through V8's compilation cache
    return new Function(
        'run',
        'repetitions',
        `let result;
        const start = performance.now();
        for (let i = 0; i < repetitions; i++) {
            result = run();
        }
        const elapsed = performance.now() - start;
        globalThis.benchmarkSink = result;
        return (elapsed / repetitions) * 1000; // timer ${timerId++}`
    ) as Timer;
}

/** The repetitions that make one sample take about `sampleTime` */
function calibrate({ run }: Variant, sampleTime: number): number {
    let repetitions = 1;
    for (;;) {
        const start = performance.now();
        for (let i = 0; i < repetitions; i++) {
            run();
        }
        const elapsed = performance.now() - start;

        if (elapsed >= sampleTime / 4 || repetitions >= 1 << 20) {
            return Math.max(
                1,
                Math.round((repetitions * sampleTime) / Math.max(elapsed, 1e-3))
            );
        }

        repetitions *= 2;
    }
}
