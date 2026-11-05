import { useEffect, useId, useRef, useState } from "react";
import { initials } from "./api";
import ThemeControl from "./ThemeControl";
import { Icon } from "./ui";
import "./profileMenu.css";

type Props = {
  name: string;
  roleLabel: string;
  canOpenSettings: boolean;
  onOpenSettings: () => boolean;
  onLeaveLocalOperator: () => boolean;
};

/** Disclosure controls existing local routes; it grants no new permissions. */
export default function ProfileMenu({ name, roleLabel, canOpenSettings, onOpenSettings, onLeaveLocalOperator }: Props) {
  const [open, setOpen] = useState(false);
  const [box, setBox] = useState({ left: 8, bottom: 8, width: 280, maxHeight: 400 });
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const panelId = useId();
  function close(restoreFocus = false) {
    setOpen(false);
    if (restoreFocus) trigger.current?.focus();
  }
  function position() {
    const rect = trigger.current?.getBoundingClientRect();
    if (!rect) return;
    const width = Math.min(300, window.innerWidth - 16);
    setBox({ left: Math.max(8, Math.min(rect.left, window.innerWidth - width - 8)),
      bottom: Math.max(8, window.innerHeight - rect.top + 8), width,
      maxHeight: Math.max(80, rect.top - 16) });
  }
  useEffect(() => {
    if (!open) return;
    function outside(event: Event) {
      if (event.target instanceof Node && !root.current?.contains(event.target)) setOpen(false);
    }
    function escape(event: KeyboardEvent) {
      if (event.key !== "Escape" || !(event.target instanceof Node) || !root.current?.contains(event.target)) return;
      event.preventDefault();
      event.stopPropagation();
      setOpen(false);
      trigger.current?.focus();
    }
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside, true);
    document.addEventListener("keydown", escape, true);
    window.addEventListener("resize", position);
    window.addEventListener("scroll", position, true);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside, true);
      document.removeEventListener("keydown", escape, true);
      window.removeEventListener("resize", position);
      window.removeEventListener("scroll", position, true);
    };
  }, [open]);

  return <div className="profile-menu" ref={root}>
    <button ref={trigger} type="button" className="profile-trigger" aria-expanded={open}
      aria-controls={panelId} aria-label={`Profile and settings for ${name || "local operator"}`}
      onClick={() => { if (open) close(); else { position(); setOpen(true); } }}>
      <span className="profile-avatar" aria-hidden="true">{initials(name) || "N"}</span>
      <span className="profile-identity"><strong>{name || "Local operator"}</strong><small>{roleLabel}</small></span>
      <Icon name="chevron" size={16} />
    </button>
    {/* Keep ThemeControl mounted so system-theme changes still work when closed. */}
    <div id={panelId} className="profile-panel" hidden={!open} style={box}>
      <strong>Profile and settings</strong>
      <ThemeControl />
      {canOpenSettings && <button type="button" className="button secondary"
        onClick={() => { if (onOpenSettings()) close(); }}>Workspace settings</button>}
      <button type="button" className="button secondary" onClick={() => { if (onLeaveLocalOperator()) close(); }}>
        Switch or leave workspace
      </button>
      <small>Return to local operator selection.</small>
    </div>
  </div>;
}
