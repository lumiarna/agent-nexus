import { mergeConfig } from "vite";
import { defineConfig } from "vitest/config";
import viteConfig from "./vite.config";

// Component tests run on vitest (jsdom + RTL) and reuse the app's vite config,
// so the `@` alias and the React JSX transform work for free. Pure-logic tests
// stay on `node:test` (`pnpm test:unit`); this runner is scoped to
// tests/component/ so the two suites never pick up each other's files.
export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      environment: "jsdom",
      include: ["tests/component/**/*.test.tsx"],
      globals: false,
    },
  }),
);
