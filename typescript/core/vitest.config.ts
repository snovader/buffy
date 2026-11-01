import { defineProject } from 'vitest/config';

const config = defineProject({
    test: {
        pool: 'threads',
        include: ['test/**/*.test.ts'],
        // Resolve the package and its `#` imports to its source rather than
        // its build
        execArgv: ['--conditions=@buffy/source'],
        // Run tests in Node's own module loader, as the package runs, rather
        // than in Vite's
        experimental: {
            viteModuleRunner: false
        }
    }
});

export default config;
