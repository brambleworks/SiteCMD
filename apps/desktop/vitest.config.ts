import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

const configDir = import.meta.dirname;

export default defineConfig({
  plugins: [react()],
  resolve: {
    alias: {
      "@": path.resolve(configDir, "./src"),
    },
  },
  test: {
    environment: "jsdom",
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    maxWorkers: process.env.CI ? "50%" : 4,
    globals: false,
    setupFiles: ["./src/test/setup.ts"],
  },
});
