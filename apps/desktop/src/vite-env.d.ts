/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly VITE_APP_VERSION?: string;
  readonly VITE_SOURCE_COMMIT?: string;
  readonly VITE_SITECMD_TELEMETRY_ENDPOINT?: string;
  readonly VITE_SITECMD_SENTRY_DSN?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
