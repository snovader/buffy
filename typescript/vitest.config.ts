import { defineConfig } from 'vitest/config';

const config = defineConfig({
    test: {
        projects: ['core']
    }
});

export default config;
