import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";
import { NxIcon, type NexusIconName } from "./Icon";
import { Scene, type NexusSceneName } from "./Scene";
import type { Tone } from "./status";

const cx = (...parts: (string | false | null | undefined)[]) => parts.filter(Boolean).join(" ");

/* ---------------------------------------------------------------- Button */

export type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  /** One primary button per view. Destructive actions are named for what they do. */
  variant?: "primary" | "secondary" | "quiet" | "danger";
  size?: "md" | "sm";
  icon?: NexusIconName;
  /** Shows the sun and keeps the button's width. */
  busy?: boolean;
};

export function NxButton({ variant = "primary", size = "md", icon, busy = false, className, children, disabled, type = "button", ...rest }: ButtonProps) {
  return (
    <button
      type={type}
      className={cx("nx-button", `nx-button--${variant}`, size === "sm" && "nx-button--sm", busy && "is-busy", className)}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      {...rest}
    >
      {busy ? <NxIcon name="sun" size={16} className="nx-spin" /> : icon ? <NxIcon name={icon} size={16} /> : null}
      {children}
    </button>
  );
}

/* ----------------------------------------------------------------- Badge */

export function NxBadge({ tone = "neutral", icon, children, className }: { tone?: Tone; icon?: NexusIconName; children: ReactNode; className?: string }) {
  return (
    <span className={cx("nx-badge", `nx-badge--${tone}`, className)}>
      {icon && <NxIcon name={icon} size={13} />}
      {children}
    </span>
  );
}

/* ------------------------------------------------------------- Leaf card */

export type LeafCardProps = HTMLAttributes<HTMLElement> & {
  as?: "div" | "section" | "article";
  size?: "sm" | "md" | "lg";
  selected?: boolean;
  /** Adds the hover state; use when the whole card opens something. */
  interactive?: boolean;
};

/** The default card: tip top left, stem bottom right, a faint vein under the text. */
export function LeafCard({ as: Tag = "div", size = "md", selected = false, interactive = false, className, children, ...rest }: LeafCardProps) {
  return (
    <Tag className={cx("nx-leaf", size !== "md" && `nx-leaf--${size}`, selected && "is-selected", interactive && "is-interactive", className)} {...rest}>
      {children}
    </Tag>
  );
}

export function MetricCard({ label, value, context, icon, tone = "neutral", className }: {
  label: string;
  value: ReactNode;
  /** One line under the figure. Colour belongs here, never on the number. */
  context?: ReactNode;
  icon?: NexusIconName;
  tone?: Tone;
  className?: string;
}) {
  return (
    <LeafCard className={cx("nx-metric", className)}>
      <span className="nx-metric-label">
        {label}
        {icon && <NxIcon name={icon} size={16} />}
      </span>
      <strong className="nx-metric-value">{value}</strong>
      {context && <span className={cx("nx-metric-context", tone !== "neutral" && `nx-tone--${tone}`)}>{context}</span>}
    </LeafCard>
  );
}

/* ----------------------------------------------------------------- Field */

export function Field({ label, hint, error, htmlFor, required, children, className }: {
  label: string;
  hint?: ReactNode;
  /** A sentence that says what to enter. The typed text is kept. */
  error?: ReactNode;
  htmlFor?: string;
  required?: boolean;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cx("nx-field", error ? "has-error" : null, className)}>
      <label htmlFor={htmlFor}>
        {label}
        {required && <span className="nx-field-required"> (required)</span>}
      </label>
      {children}
      {error ? (
        <span className="nx-field-error" role="alert">
          <NxIcon name="error" size={14} />
          {error}
        </span>
      ) : hint ? (
        <span className="nx-field-hint">{hint}</span>
      ) : null}
    </div>
  );
}

/* ------------------------------------------------------------------ Tabs */

export function NxTabs({ tabs, value, onChange, label = "Choose a section" }: {
  tabs: { id: string; label: string; count?: number }[];
  value: string;
  onChange: (id: string) => void;
  label?: string;
}) {
  return (
    <div className="nx-tabs" role="group" aria-label={label}>
      {tabs.map((t) => (
        <button type="button" key={t.id} aria-pressed={value === t.id} className={value === t.id ? "is-active" : undefined} onClick={() => onChange(t.id)}>
          {t.label}
          {t.count !== undefined && <span className="nx-count">{t.count}</span>}
        </button>
      ))}
    </div>
  );
}

/* ----------------------------------------------------------------- Toast */

export function Toast({ tone = "ok", children, action }: { tone?: "ok" | "bad" | "info"; children: ReactNode; action?: ReactNode }) {
  return (
    <div className="nx-toast" role="status">
      <NxIcon name={tone === "bad" ? "error" : tone === "info" ? "info" : "success"} size={16} />
      <span>{children}</span>
      {action}
    </div>
  );
}

/* ----------------------------------------------------------- Empty state */

export function EmptyState({ scene = "clear-day", title, description, action, compact = false }: {
  /** clear-day: nothing due. first-shoot: a first. waymark: nothing found. record: history. */
  scene?: NexusSceneName;
  title: string;
  /** Say what will fill the space, or what to do next. */
  description: ReactNode;
  action?: ReactNode;
  compact?: boolean;
}) {
  return (
    <div className={cx("nx-empty", compact && "nx-empty--compact")}>
      <Scene name={scene} className="nx-empty-scene" />
      <div>
        <h3>{title}</h3>
        <p>{description}</p>
        {action}
      </div>
    </div>
  );
}

/* ----------------------------------------------------------------- Panel */

export function NxPanel({ title, subtitle, action, children, className }: { title?: string; subtitle?: string; action?: ReactNode; children: ReactNode; className?: string }) {
  return (
    <section className={cx("nx-panel", className)}>
      {(title || action) && (
        <div className="nx-panel-head">
          <div>
            <h2>{title}</h2>
            {subtitle && <p>{subtitle}</p>}
          </div>
          {action}
        </div>
      )}
      {children}
    </section>
  );
}
