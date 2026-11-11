import { useEffect, useRef } from "react";
import type { ReactNode } from "react";
import { label } from "./api";
import { EmptyState, Logo, NxBadge, NxIcon, statusStyle, type NexusIconName, type NexusSceneName } from "./nexus-ui";

export type IconName =
  | "overview"
  | "clients"
  | "schedule"
  | "notes"
  | "billing"
  | "tasks"
  | "team"
  | "settings"
  | "search"
  | "plus"
  | "arrow"
  | "back"
  | "check"
  | "close"
  | "chevron"
  | "shield"
  | "activity"
  | "refresh"
  | "filter"
  | "sun"
  | "download"
  | "leaf"
  | "clock"
  | "link"
  | "menu";
/** The older icon names used across the views, mapped onto the Nexus icon set. */
const iconFor: Record<IconName, NexusIconName> = {
  overview: "overview", clients: "clients", schedule: "schedule", notes: "documentation", billing: "billing",
  tasks: "tasks", team: "group", settings: "settings", search: "search", plus: "add", arrow: "arrow-right",
  back: "arrow-left", check: "check", close: "close", chevron: "chevron-right", shield: "shield",
  activity: "reporting", refresh: "refresh", filter: "filter", sun: "sun", download: "download",
  leaf: "care-plan", clock: "clock", link: "link", menu: "menu",
};
export function Icon({ name, size = 20 }: { name: IconName; size?: number }) {
  return <NxIcon name={iconFor[name]} size={size} />;
}
export function Brand() {
  return (
    <div className="brand">
      <Logo size={34} />
    </div>
  );
}
export function Badge({ status }: { status: unknown }) {
  const s = String(status ?? "");
  const key = s.replace(/[^a-z_]/g, "");
  const { tone, icon } = statusStyle(key);
  return (
    <NxBadge tone={tone} icon={icon} className={`badge badge-${key}`}>
      {label(s || "Not set")}
    </NxBadge>
  );
}
/** Which illustration fits an empty state, from the icon the view asked for. */
const sceneFor: Partial<Record<IconName, NexusSceneName>> = {
  search: "waymark", filter: "waymark", notes: "record", billing: "record", download: "record",
  clock: "record", activity: "record", plus: "first-shoot", clients: "first-shoot", team: "first-shoot",
};
export function Empty({
  icon = "leaf",
  title,
  description,
  action,
}: {
  icon?: IconName;
  title: string;
  description: string;
  action?: ReactNode;
}) {
  return <EmptyState scene={sceneFor[icon] ?? "clear-day"} title={title} description={description} action={action} />;
}
export function Button({
  children,
  onClick,
  variant = "",
  disabled = false,
  type = "button",
  className = "",
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: string;
  disabled?: boolean;
  type?: "button" | "submit";
  className?: string;
}) {
  return (
    <button
      type={type}
      className={`button ${variant} ${className}`}
      disabled={disabled}
      onClick={onClick}
    >
      {children}
    </button>
  );
}
export function Panel({
  title,
  subtitle,
  action,
  children,
  className = "",
}: {
  title?: string;
  subtitle?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`panel ${className}`}>
      {(title || action) && (
        <div className="panel-head">
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
export function Modal({
  title,
  description,
  children,
  onClose,
  wide = false,
}: {
  title: string;
  description?: string;
  children: ReactNode;
  onClose: () => void;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    ref.current?.showModal();
    const old = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = old;
    };
  }, []);
  return (
    <dialog
      ref={ref}
      className={`modal ${wide ? "wide" : ""}`}
      onCancel={event => { event.preventDefault(); onClose(); }}
      aria-labelledby="modal-title"
    >
      <div className="modal-head">
        <div>
          <span className="eyebrow">NEXUS WORKSPACE</span>
          <h2 id="modal-title">{title}</h2>
          {description && <p>{description}</p>}
        </div>
        <button
          type="button"
          className="icon-button"
          aria-label="Close dialog"
          onClick={onClose}
        >
          <Icon name="close" />
        </button>
      </div>
      {children}
    </dialog>
  );
}
export function Tabs({
  tabs,
  value,
  onChange,
}: {
  tabs: { id: string; label: string; count?: number }[];
  value: string;
  onChange: (id: string) => void;
}) {
  return (
    <div className="tabs" role="group" aria-label="Choose a section">
      {tabs.map((t) => (
        <button
          type="button"
          aria-pressed={value === t.id}
          className={value === t.id ? "active" : ""}
          key={t.id}
          onClick={() => onChange(t.id)}
        >
          {t.label}
          {t.count !== undefined && <span>{t.count}</span>}
        </button>
      ))}
    </div>
  );
}
