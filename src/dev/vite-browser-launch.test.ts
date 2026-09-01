import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { EventEmitter } from "node:events";
import type { MinimalPluginContextWithoutEnvironment, ViteDevServer } from "vite";
import { createWebDevelopmentBrowserPlugin } from "./vite-browser-launch";

const browserLaunchStateKey = Symbol.for("dev.agent-switchboard.web-development-browser-launch");

function configure(plugin: ReturnType<typeof createWebDevelopmentBrowserPlugin>, server: ViteDevServer) {
  const hook = plugin.configureServer;
  if (typeof hook !== "function") throw new Error("configureServer hook is required");
  hook.call({} as MinimalPluginContextWithoutEnvironment, server);
}

function developmentServer(openBrowser: () => void) {
  const httpServer = Object.assign(new EventEmitter(), { listening: true });
  return {
    httpServer,
    server: { httpServer, openBrowser } as unknown as ViteDevServer,
  };
}

function resetBrowserLaunchState() {
  const registry = globalThis as typeof globalThis & Record<symbol, unknown>;
  delete registry[browserLaunchStateKey];
}

describe("web development browser launch", () => {
  beforeEach(() => {
    resetBrowserLaunchState();
    vi.useFakeTimers();
  });

  afterEach(() => {
    resetBrowserLaunchState();
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  it("waits for the real backend and opens once for the Vite process", async () => {
    const fetchMock = vi
      .fn()
      .mockResolvedValueOnce({ status: 503 })
      .mockResolvedValueOnce({ status: 204 });
    vi.stubGlobal("fetch", fetchMock);
    const openBrowser = vi.fn();
    const { server } = developmentServer(openBrowser);
    const options = {
      enabled: true,
      healthUrl: "http://127.0.0.1:1422/health",
      origin: "http://127.0.0.1:1420",
      retryDelayMs: 10,
    };

    configure(createWebDevelopmentBrowserPlugin(options), server);
    await vi.advanceTimersByTimeAsync(10);

    expect(fetchMock).toHaveBeenCalledTimes(2);
    expect(fetchMock).toHaveBeenLastCalledWith(options.healthUrl, {
      cache: "no-store",
      headers: { Origin: options.origin },
    });
    expect(openBrowser).toHaveBeenCalledTimes(1);

    configure(createWebDevelopmentBrowserPlugin(options), server);
    await vi.runAllTimersAsync();
    expect(openBrowser).toHaveBeenCalledTimes(1);
  });

  it("does nothing for desktop development", async () => {
    const fetchMock = vi.fn();
    vi.stubGlobal("fetch", fetchMock);
    const openBrowser = vi.fn();

    configure(
      createWebDevelopmentBrowserPlugin({
        enabled: false,
        healthUrl: "http://127.0.0.1:1422/health",
        origin: "http://127.0.0.1:1420",
      }),
      developmentServer(openBrowser).server,
    );
    await vi.runAllTimersAsync();

    expect(fetchMock).not.toHaveBeenCalled();
    expect(openBrowser).not.toHaveBeenCalled();
  });

  it("lets a replacement Vite server take over a pending launch", async () => {
    let resolveFirstCheck: ((value: { status: number }) => void) | undefined;
    const fetchMock = vi
      .fn()
      .mockImplementationOnce(
        () =>
          new Promise<{ status: number }>((resolve) => {
            resolveFirstCheck = resolve;
          }),
      )
      .mockResolvedValueOnce({ status: 204 });
    vi.stubGlobal("fetch", fetchMock);
    const firstOpen = vi.fn();
    const secondOpen = vi.fn();
    const options = {
      enabled: true,
      healthUrl: "http://127.0.0.1:1422/health",
      origin: "http://127.0.0.1:1420",
    };

    configure(createWebDevelopmentBrowserPlugin(options), developmentServer(firstOpen).server);
    await vi.advanceTimersByTimeAsync(0);
    configure(createWebDevelopmentBrowserPlugin(options), developmentServer(secondOpen).server);
    await vi.advanceTimersByTimeAsync(0);

    expect(secondOpen).toHaveBeenCalledTimes(1);
    expect(firstOpen).not.toHaveBeenCalled();

    resolveFirstCheck?.({ status: 204 });
    await vi.advanceTimersByTimeAsync(0);
    expect(firstOpen).not.toHaveBeenCalled();
  });

  it("cancels a pending launch when its Vite server closes", async () => {
    const fetchMock = vi.fn().mockResolvedValue({ status: 503 });
    vi.stubGlobal("fetch", fetchMock);
    const openBrowser = vi.fn();
    const { server, httpServer } = developmentServer(openBrowser);

    configure(
      createWebDevelopmentBrowserPlugin({
        enabled: true,
        healthUrl: "http://127.0.0.1:1422/health",
        origin: "http://127.0.0.1:1420",
        retryDelayMs: 10,
      }),
      server,
    );
    await vi.advanceTimersByTimeAsync(0);
    httpServer.emit("close");
    await vi.advanceTimersByTimeAsync(20);

    expect(fetchMock).toHaveBeenCalledTimes(1);
    expect(openBrowser).not.toHaveBeenCalled();
  });
});
