export type AppConfig = {
  API_URL: string;
  MAP_STYLE_URL: string;
  LOCAL_ADMIN_AUTO_LOGIN: boolean;
};

declare global {
  interface Window {
    __APP_CONFIG__?: Partial<AppConfig>;
  }
}

const DEFAULT_CONFIG: AppConfig = {
  API_URL: "http://localhost:3000/api",
  MAP_STYLE_URL: "opentopomap",
  LOCAL_ADMIN_AUTO_LOGIN: false,
};

let clientConfig: AppConfig | null = null;

function normalizeUrl(value: string | undefined, fallback: string): string {
  const trimmed = value?.trim();
  return trimmed && trimmed.length > 0 ? trimmed : fallback;
}

function normalizeBoolean(
  value: boolean | string | undefined,
  fallback: boolean,
): boolean {
  if (typeof value === "boolean") {
    return value;
  }

  const normalized = value?.trim().toLowerCase();
  if (!normalized) {
    return fallback;
  }

  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }

  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }

  return fallback;
}

export function parseAppConfig(raw?: Partial<AppConfig>): AppConfig {
  return {
    API_URL: normalizeUrl(raw?.API_URL, DEFAULT_CONFIG.API_URL),
    MAP_STYLE_URL: normalizeUrl(
      raw?.MAP_STYLE_URL,
      DEFAULT_CONFIG.MAP_STYLE_URL,
    ),
    LOCAL_ADMIN_AUTO_LOGIN: normalizeBoolean(
      raw?.LOCAL_ADMIN_AUTO_LOGIN,
      DEFAULT_CONFIG.LOCAL_ADMIN_AUTO_LOGIN,
    ),
  };
}

export function getServerConfig(): AppConfig {
  return {
    API_URL: normalizeUrl(process.env.API_URL, DEFAULT_CONFIG.API_URL),
    MAP_STYLE_URL: normalizeUrl(
      process.env.MAP_STYLE_URL,
      DEFAULT_CONFIG.MAP_STYLE_URL,
    ),
    LOCAL_ADMIN_AUTO_LOGIN: normalizeBoolean(
      process.env.LOCAL_ADMIN_AUTO_LOGIN,
      DEFAULT_CONFIG.LOCAL_ADMIN_AUTO_LOGIN,
    ),
  };
}

function configChanged(current: AppConfig, next: AppConfig): boolean {
  return (
    current.API_URL !== next.API_URL ||
    current.MAP_STYLE_URL !== next.MAP_STYLE_URL ||
    current.LOCAL_ADMIN_AUTO_LOGIN !== next.LOCAL_ADMIN_AUTO_LOGIN
  );
}

export function initializeClientConfig(
  initialConfig?: Partial<AppConfig>,
): AppConfig {
  if (initialConfig) {
    clientConfig = parseAppConfig(initialConfig);
    return clientConfig;
  }

  if (typeof window !== "undefined") {
    const windowConfig = parseAppConfig(window.__APP_CONFIG__);

    if (
      !clientConfig ||
      (window.__APP_CONFIG__ && configChanged(clientConfig, windowConfig))
    ) {
      clientConfig = windowConfig;
    }

    return clientConfig;
  }

  if (!clientConfig) {
    clientConfig = getServerConfig();
  }

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
