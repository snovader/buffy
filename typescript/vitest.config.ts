import { defineConfig } from 'vitest/config';

const config = defineConfig({
    test: {
        projects: ['benchmarks', 'core']
    }
});

export default config;
