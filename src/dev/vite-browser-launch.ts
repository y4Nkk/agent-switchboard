import type { Plugin, ViteDevServer } from "vite";

const browserLaunchStateKey = Symbol.for("dev.agent-switchboard.web-development-browser-launch");

type BrowserLaunchState = {
  generation: number;
  status: "idle" | "waiting" | "opened";
};

type BrowserLaunchOptions = {
  enabled: boolean;
  healthUrl: string;
  origin: string;
  retryDelayMs?: number;
};

async function defaultBackendReady(healthUrl: string, origin: string): Promise<boolean> {
  try {
    const response = await fetch(healthUrl, {
      cache: "no-store",
      headers: { Origin: origin },
    });
    return response.status === 204;
  } catch {
    return false;
  }
}

function delay(milliseconds: number): Promise<void> {
  return new Promise((resolve) => globalThis.setTimeout(resolve, milliseconds));
}

function browserLaunchState(): BrowserLaunchState {
  const registry = globalThis as typeof globalThis & Record<symbol, unknown>;
  const existing = registry[browserLaunchStateKey] as BrowserLaunchState | undefined;
  if (existing) return existing;

  const state: BrowserLaunchState = { generation: 0, status: "idle" };
  registry[browserLaunchStateKey] = state;
  return state;
}

async function waitUntilListening(server: ViteDevServer): Promise<boolean> {
  const httpServer = server.httpServer;
  if (!httpServer || httpServer.listening) return true;
  return new Promise<boolean>((resolve) => {
    let settled = false;
    const finish = (listening: boolean) => {
      if (settled) return;
      settled = true;
      resolve(listening);
    };
    httpServer.once("listening", () => finish(true));
    httpServer.once("close", () => finish(false));
  });
}

export function createWebDevelopmentBrowserPlugin({
  enabled,
  healthUrl,
  origin,
  retryDelayMs = 100,
}: BrowserLaunchOptions): Plugin {
  return {
    name: "agent-switchboard-web-development-browser",
    apply: "serve",
    configureServer(server) {
      if (!enabled) return;

      const state = browserLaunchState();
      if (state.status === "opened") return;
      const generation = state.generation + 1;
      state.generation = generation;
      state.status = "waiting";
      const isCurrent = () => state.generation === generation && state.status === "waiting";

      server.httpServer?.once("close", () => {
        if (!isCurrent()) return;
        state.generation += 1;
        state.status = "idle";
      });

      void (async () => {
        if (!(await waitUntilListening(server)) || !isCurrent()) return;
        while (isCurrent()) {
          const ready = await defaultBackendReady(healthUrl, origin);
          if (!isCurrent()) return;
          if (ready) {
            state.status = "opened";
            server.openBrowser();
            return;
          }
          await delay(retryDelayMs);
        }
      })();
    },
  };
}
