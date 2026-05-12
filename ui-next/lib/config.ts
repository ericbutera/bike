export type AppConfig = {
  API_URL: string;
};

declare global {
  interface Window {
    __APP_CONFIG__?: Partial<AppConfig>;
  }
}

const DEFAULT_CONFIG: AppConfig = {
  API_URL: "http://localhost:3000/api",
};

let clientConfig: AppConfig | null = null;

function normalizeUrl(value: string | undefined, fallback: string): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : fallback;
}

export function parseAppConfig(raw?: Partial<AppConfig>): AppConfig {
  return {
    API_URL: normalizeUrl(raw?.API_URL, DEFAULT_CONFIG.API_URL),
  };
}

export function getServerConfig(): AppConfig {
  return {
    API_URL: normalizeUrl(process.env.API_URL, DEFAULT_CONFIG.API_URL),
  };
}

export function initializeClientConfig(
  initialConfig?: Partial<AppConfig>,
): AppConfig {
  if (clientConfig) {
    return clientConfig;
  }

  if (initialConfig) {
    clientConfig = parseAppConfig(initialConfig);
    return clientConfig;
  }

  if (typeof window !== "undefined") {
    clientConfig = parseAppConfig(window.__APP_CONFIG__);
    return clientConfig;
  }

  clientConfig = getServerConfig();
  return clientConfig;
}

export function getClientConfig(): AppConfig {
  return initializeClientConfig();
}

export const config: AppConfig = new Proxy(DEFAULT_CONFIG, {
  get(_target, prop: keyof AppConfig) {
    return getClientConfig()[prop];
  },
});
