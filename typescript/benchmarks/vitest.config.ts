import { defineProject } from 'vitest/config';

const config = defineProject({
    test: {
        include: [],
        benchmark: {
            include: ['*.bench.ts']
        },
        // Resolve @buffy/core to its source rather than its build
        execArgv: ['--conditions=@buffy/source'],
        experimental: {
            viteModuleRunner: false
        }
    }
});

export default config;
