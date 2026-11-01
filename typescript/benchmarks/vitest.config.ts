import { defineProject } from 'vitest/config';

const config = defineProject({
    test: {
        include: [],
        benchmark: {
            include: ['*.bench.ts']
        },
        // Resolve tafla to its source rather than its build
        execArgv: ['--conditions=tafla-source'],
        experimental: {
            viteModuleRunner: false
        }
    }
});

export default config;
