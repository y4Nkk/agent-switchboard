/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  build: {
    target: "es2022",
    outDir: "dist",
  },
  test: {
    environment: "jsdom",
    setupFiles: ["src/test/setup.ts"],
    include: ["src/**/*.test.{ts,tsx}"],
    // timeLabel renders in the machine timezone; pin it so local-time
    // assertions are identical on dev machines (UTC+8) and CI runners (UTC).
    env: { TZ: "Asia/Shanghai" },
  },
});
