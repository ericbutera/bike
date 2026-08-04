type MetricCardVariant = "stat" | "plain";
type MetricCardSize = "sm" | "md" | "lg";
type MetricCardTone = "base-100" | "base-200";

const statValueClassNames: Record<MetricCardSize, string> = {
  sm: "text-base",
  md: "text-lg sm:text-xl",
  lg: "text-2xl",
};

const statPaddingClassNames: Record<MetricCardSize, string> = {
  sm: "px-3 py-2",
  md: "px-4 py-4",
  lg: "px-4 py-4",
};

const statToneClassNames: Record<MetricCardTone, string> = {
  "base-100": "bg-base-100",
  "base-200": "bg-base-200",
};

export default function MetricCard({
  label,
  value,
  variant = "stat",
  size = "md",
  tone = "base-200",
  shadow = true,
  className = "",
}: {
  label: string;
  value: string;
  variant?: MetricCardVariant;
  size?: MetricCardSize;
  tone?: MetricCardTone;
  shadow?: boolean;
  className?: string;
}) {
  if (variant === "plain") {
    return (
      <div className={`min-w-0 ${className}`.trim()}>
        <div className="text-2xl font-semibold text-base-content">{value}</div>
        <div className="mt-1 text-xs text-base-content/55 uppercase">
          {label}
        </div>
      </div>
    );
  }

  return (
    <div
      className={`stats border border-base-300 ${statToneClassNames[tone]} ${shadow ? "shadow-sm" : ""} ${className}`.trim()}
    >
      <div className={`stat ${statPaddingClassNames[size]}`}>
        <div className="stat-title">{label}</div>
        <div className={`stat-value ${statValueClassNames[size]}`}>
          {value}
        </div>
      </div>
    </div>
  );
}
