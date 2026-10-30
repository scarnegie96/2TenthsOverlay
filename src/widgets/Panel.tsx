import type { ReactNode } from "react";

/** The shared widget chrome: a dark glass panel with an optional header line. */
export function Panel({
  title,
  aside,
  width,
  children,
  className = "",
}: {
  title?: ReactNode;
  aside?: ReactNode;
  width?: number;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`panel ${className}`} style={width ? { width } : undefined}>
      {(title || aside) && (
        <header className="panel-head">
          <span className="panel-title">{title}</span>
          {aside && <span className="panel-aside">{aside}</span>}
        </header>
      )}
      {children}
    </section>
  );
}

export function Empty({ children }: { children: ReactNode }) {
  return <div className="panel-empty">{children}</div>;
}
