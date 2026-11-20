// SPDX-License-Identifier: MPL-2.0
import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    projects: [
      { test: { name: 'node', include: ['src/**/*.test.ts'], environment: 'node' } },
      { test: { name: 'dom', include: ['src/**/*.test.tsx'], environment: 'jsdom' } },
    ],
  },
});
