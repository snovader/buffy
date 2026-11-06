// The values that `core/vitest.config.ts` provides, which tests read with
// `inject()`
declare module 'vitest' {
    export interface ProvidedContext {
        /** Whether to run tests that allocate buffers of more than 4 GiB */
        largeBufferTests: boolean;
    }
}

export {};
