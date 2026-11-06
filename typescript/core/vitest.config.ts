import { defineProject } from 'vitest/config';

const config = defineProject({
    test: {
        pool: 'threads',
        include: ['test/**/*.test.ts'],
        // Resolve the package and its `#` imports to its source rather than
        // its build
        execArgv: ['--conditions=tafla-source'],
        // Run tests in Node's own module loader, as the package runs, rather
        // than in Vite's
        experimental: {
            viteModuleRunner: false
        },
        // The tests can't read the environment themselves, as they're typed
        // without Node's globals
        provide: {
            // Tests that allocate buffers of more than 4 GiB
            largeBufferTests: process.env.TAFLA_LARGE_BUFFER_TESTS === '1'
        }
    }
});

export default config;
