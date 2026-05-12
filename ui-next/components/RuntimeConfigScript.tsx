import type { AppConfig } from "@/lib/config";

function escapeScriptValue(value: string): string {
  return value.replace(/</g, "\\u003c");
}

export default function RuntimeConfigScript({ config }: { config: AppConfig }) {
  return (
    <script
      dangerouslySetInnerHTML={{
        __html: `window.__APP_CONFIG__=${escapeScriptValue(JSON.stringify(config))};`,
      }}
    />
  );
}
