import type { ComponentPropsWithoutRef, ReactNode } from "react";

type CardElement = "article" | "div" | "section";

type AppCardProps = ComponentPropsWithoutRef<"div"> & {
  as?: CardElement;
  bodyClassName?: string;
};

type CardHeaderProps = Omit<ComponentPropsWithoutRef<"div">, "title"> & {
  actions?: ReactNode;
  description?: ReactNode;
  title?: ReactNode;
  titleExtras?: ReactNode;
};

function cx(...classNames: Array<string | false | null | undefined>) {
  return classNames.filter(Boolean).join(" ");
}

function AppCard({
  as: Component = "div",
  bodyClassName,
  children,
  className,
  ...props
}: AppCardProps) {
  return (
    <Component
      className={cx("card bg-base-100 shadow-xl", className)}
      {...props}
    >
      <div className={cx("card-body", bodyClassName)}>{children}</div>
    </Component>
  );
}

function CardHeader({
  actions,
  children,
  className,
  description,
  title,
  titleExtras,
  ...props
}: CardHeaderProps) {
  return (
    <div
      className={cx("flex items-start justify-between gap-3", className)}
      {...props}
    >
      <div className="min-w-0">
        {children ?? (
          <>
            <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-[0.24em] text-base-content/50">
              <h2>{title}</h2>
              {titleExtras}
            </div>
            {description ? (
              <p className="mt-1 text-sm text-base-content/70">{description}</p>
            ) : null}
          </>
        )}
      </div>
      {actions ? (
        <div className="flex flex-wrap items-center justify-end gap-2">
          {actions}
        </div>
      ) : null}
    </div>
  );
}

export { AppCard, CardHeader };
