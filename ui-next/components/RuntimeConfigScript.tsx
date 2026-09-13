import type { AppConfig } from "@/lib/config";
import type { RequestContextHeaders } from "@/lib/trace-context";

function escapeScriptValue(value: string): string {
  return value.replace(/</g, "\\u003c");
}

export default function RuntimeConfigScript({
  config,
  requestContext,
}: {
  config: AppConfig;
  requestContext: RequestContextHeaders;
}) {
  return (
    <script
      dangerouslySetInnerHTML={{
        __html: [
          `window.__APP_CONFIG__=${escapeScriptValue(JSON.stringify(config))};`,
          `window.__REQUEST_CONTEXT__=${escapeScriptValue(JSON.stringify(requestContext))};`,
        ].join(""),
      }}
    />
  );
}
