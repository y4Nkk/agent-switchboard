/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { createWebDevelopmentBrowserPlugin } from "./src/dev/vite-browser-launch.ts";
import tauriConfig from "./src-tauri/tauri.conf.json" with { type: "json" };

const developmentUrl = new URL(tauriConfig.build.devUrl);
const browserDevelopment = Object.freeze({
  origin: developmentUrl.origin,
  host: developmentUrl.hostname,
  port: Number(developmentUrl.port),
});

export default defineConfig({
  plugins: [
    react(),
    createWebDevelopmentBrowserPlugin({
      enabled: process.env.ASB_WEB_DEVELOPMENT === "1",
      healthUrl: "http://127.0.0.1:1422/health",
      origin: browserDevelopment.origin,
    }),
  ],
  clearScreen: false,
  server: {
    host: browserDevelopment.host,
    port: browserDevelopment.port,
    strictPort: true,
    open: false,
    proxy: {
      "/api": {
        target: "http://127.0.0.1:1422",
        changeOrigin: false,
        rewrite: (path) => path.replace(/^\/api/, ""),
      },
    },
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
