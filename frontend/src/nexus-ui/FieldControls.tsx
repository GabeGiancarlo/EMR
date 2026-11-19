import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import type {
  ComponentProps,
  KeyboardEvent as ReactKey,
  ReactNode,
  RefObject,
} from "react";
import { NxIcon } from "./Icon";
import {
  addDays,
  addMonths,
  formatYmdLong,
  monthGrid,
  weekdayIndex,
  validCalendarDay,
} from "../calendar";

type Anchor = RefObject<HTMLElement | null>;

/** Keep menus in the form's DOM tree (including modal dialogs) while escaping its scroll area. */
function Flyout({
  anchor,
  close,
  children,
  id,
  role,
  label,
  className,
}: {
  anchor: Anchor;
  close: () => void;
  children: ReactNode;
  id: string;
  role: "dialog" | "listbox";
  label: string;
  className: string;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const latestClose = useRef(close);
  latestClose.current = close;
  useLayoutEffect(() => {
    const menu = panel.current;
    const control = anchor.current;
    if (!menu || !control) return;
    menu.showPopover?.();
    function place() {
      if (!menu || !control) return;
      const r = control.getBoundingClientRect();
      const width = Math.min(
        window.innerWidth - 20,
        Math.max(r.width, role === "dialog" ? 304 : 210),
      );
      menu.style.width = width + "px";
      const below = window.innerHeight - r.bottom - 12;
      const above = r.top - 12;
      const flip = below < Math.min(menu.scrollHeight, 300) && above > below;
      menu.style.maxHeight = Math.max(100, flip ? above : below) + "px";
      menu.style.left =
        Math.max(10, Math.min(r.left, window.innerWidth - width - 10)) + "px";
      menu.style.top =
        (flip ? Math.max(6, r.top - menu.offsetHeight - 4) : r.bottom + 4) +
        "px";
    }
    place();
    function outside(e: Event) {
      if (
        !menu!.contains(e.target as Node) &&
        !control!.contains(e.target as Node)
      )
        latestClose.current();
    }
    function escape(e: KeyboardEvent) {
      if (e.key === "Escape") {
        e.preventDefault();
        e.stopPropagation();
        latestClose.current();
        control!.focus();
      }
    }
    window.addEventListener("resize", place);
    window.addEventListener("scroll", place, true);
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    document.addEventListener("keydown", escape, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", place, true);
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      document.removeEventListener("keydown", escape, true);
    };
  }, [anchor, role]);
  return (
    <div
      ref={panel}
      popover="manual"
      id={id}
      role={role}
      aria-label={label}
      className={`nx-flyout ${className}`}
      onKeyDown={(e) => {
        if (e.key === "Enter" && e.target instanceof HTMLInputElement)
          e.preventDefault();
      }}
      onClick={(e) => e.preventDefault()}
    >
      {children}
    </div>
  );
}

type Option = { value: string; text: string; disabled: boolean };
export function Choice(props: ComponentProps<"select">) {
  const ref = useRef<HTMLSelectElement>(null);
  const id = useId();
  const [options, setOptions] = useState<Option[] | null>(null);
  const [active, setActive] = useState(-1);
  const prefix = useRef({ text: "", at: 0 });
  function open() {
    const select = ref.current;
    if (!select || select.matches(":disabled")) return;
    select.focus();
    setOptions(
      Array.from(select.options, (o) => ({
        value: o.value,
        text: o.text,
        disabled: o.disabled,
      })),
    );
    setActive(select.selectedIndex);
  }
  function choose(index: number) {
    const select = ref.current;
    if (
      !select ||
      !options?.[index] ||
      options[index].disabled ||
      select.matches(":disabled")
    )
      return;
    select.value = options[index].value;
    select.dispatchEvent(new Event("change", { bubbles: true }));
    setOptions(null);
    select.focus();
  }
  useEffect(() => {
    if (options && active >= 0)
      document
        .getElementById(id + "-" + active)
        ?.scrollIntoView({ block: "nearest" });
  }, [active, options, id]);
  function key(e: ReactKey<HTMLSelectElement>) {
    if (!options) {
      if (["Enter", " ", "ArrowDown", "ArrowUp", "F4"].includes(e.key)) {
        e.preventDefault();
        open();
      }
      return;
    }
    if (e.key === "Tab") {
      setOptions(null);
      return;
    }
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      choose(active);
      return;
    }
    const enabled = options
      .map((o, i) => (o.disabled ? -1 : i))
      .filter((i) => i >= 0);
    if (!enabled.length) return;
    e.preventDefault();
    if (e.key === "Home") setActive(enabled[0]);
    else if (e.key === "End") setActive(enabled[enabled.length - 1]);
    else if (["ArrowUp", "ArrowDown"].includes(e.key)) {
      const i = enabled.indexOf(active) + (e.key === "ArrowUp" ? -1 : 1);
      setActive(enabled[(i + enabled.length) % enabled.length]);
    } else if (e.key.length === 1) {
      prefix.current = {
        at: Date.now(),
        text:
          (Date.now() - prefix.current.at < 600 ? prefix.current.text : "") +
          e.key.toLowerCase(),
      };
      const found = enabled.find((i) =>
        options[i].text.toLowerCase().startsWith(prefix.current.text),
      );
      if (found !== undefined) setActive(found);
    }
  }
  return (
    <span className="nx-choice">
      <select
        {...props}
        ref={ref}
        aria-expanded={!!options}
        aria-controls={options ? id : undefined}
        aria-activedescendant={
          options && active >= 0 ? id + "-" + active : undefined
        }
        onPointerDown={(e) => {
          if (e.button === 0) {
            e.preventDefault();
            options ? setOptions(null) : open();
          }
        }}
        onMouseDown={(e) => e.preventDefault()}
        onKeyDown={key}
      />
      <NxIcon name="chevron-down" size={15} />
      {options && (
        <Flyout
          id={id}
          anchor={ref}
          close={() => setOptions(null)}
          role="listbox"
          label={
            props["aria-label"] ||
            ref.current?.labels?.[0]?.firstChild?.textContent?.trim() ||
            "Choices"
          }
          className="nx-options"
        >
          {options.map((o, i) => (
            <div
              id={id + "-" + i}
              key={o.value + "-" + i}
              role="option"
              aria-selected={ref.current?.value === o.value}
              aria-disabled={o.disabled}
              data-active={i === active}
              onPointerMove={() => !o.disabled && setActive(i)}
              onMouseDown={(e) => e.preventDefault()}
              onClick={(e) => {
                e.preventDefault();
                choose(i);
              }}
            >
              <span>{o.text}</span>
              {ref.current?.value === o.value && (
                <NxIcon name="check" size={15} />
              )}
            </div>
          ))}
        </Flyout>
      )}
    </span>
  );
}

function localToday() {
  const d = new Date();
  return [
    d.getFullYear(),
    String(d.getMonth() + 1).padStart(2, "0"),
    String(d.getDate()).padStart(2, "0"),
  ].join("-");
}

type InputProps = ComponentProps<"input">;
export function FieldInput(props: InputProps) {
  return props.type === "date" ? (
    <DateControl {...props} />
  ) : (
    <input {...props} />
  );
}
export function DateTimeField({
  id,
  label,
  value,
  onChange,
  required,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (next: string) => void;
  required?: boolean;
}) {
  const [day = "", time = ""] = value.split("T");
  function change(nextDay: string, nextTime: string) {
    onChange(nextDay || nextTime ? nextDay + "T" + nextTime : "");
  }
  return (
    <span className="nx-datetime">
      <DateControl
        id={id}
        aria-label={label + " date"}
        required={required || !!time}
        value={day}
        onChange={(e) => change(e.target.value, time)}
      />
      <input
        aria-label={label + " time (24-hour)"}
        value={time}
        required={required || !!day}
        placeholder="HH:MM"
        title="24-hour time, HH:MM"
        pattern="([01][0-9]|2[0-3]):[0-5][0-9]"
        maxLength={5}
        onChange={(e) => change(day, e.target.value)}
      />
    </span>
  );
}
function DateControl({ min, max, ...props }: InputProps) {
  const ref = useRef<HTMLInputElement>(null);
  const [fieldName, setFieldName] = useState("date");
  useLayoutEffect(() => {
    const label = ref.current?.labels?.[0]?.cloneNode(true) as
      | HTMLElement
      | undefined;
    label
      ?.querySelectorAll("input,button,select,[popover]")
      .forEach((node) => node.remove());
    setFieldName(
      props["aria-label"] ||
        label?.textContent?.replace(/\s*\*\s*$/, "").trim() ||
        "date",
    );
  }, [props.id, props["aria-label"]]);
  const trigger = useRef<HTMLButtonElement>(null);
  const id = useId();
  const [month, setMonth] = useState("");
  const value = String(props.value ?? "");
  const datePart = value.slice(0, 10);
  const lower = String(min ?? "");
  const upper = String(max ?? "");
  const inRange = (day: string) =>
    (!lower || day >= lower.slice(0, 10)) &&
    (!upper || day <= upper.slice(0, 10));
  useLayoutEffect(() => {
    const invalid =
      value &&
      (!validCalendarDay(value) ||
        (lower && value < lower) ||
        (upper && value > upper));
    ref.current?.setCustomValidity(
      invalid ? "Enter a valid date within the allowed range." : "",
    );
  }, [value, lower, upper]);
  function open() {
    if (month) {
      setMonth("");
      return;
    }
    let day = validCalendarDay(datePart) ? datePart : localToday();
    if (upper && day > upper) day = upper.slice(0, 10);
    if (lower && day < lower) day = lower.slice(0, 10);
    setMonth(day.slice(0, 7) + "-01");
  }
  function choose(day: string) {
    const input = ref.current;
    if (!input) return;
    // Dispatch through the original input so the form's change, validation and dirty handlers run.
    Object.getOwnPropertyDescriptor(
      HTMLInputElement.prototype,
      "value",
    )!.set!.call(input, day);
    input.dispatchEvent(new Event("input", { bubbles: true }));
    setMonth("");
    input.focus();
  }
  return (
    <span className="nx-date">
      <input
        {...props}
        ref={ref}
        type="text"
        placeholder="YYYY-MM-DD"
        onKeyDown={(e) => {
          props.onKeyDown?.(e);
          if (e.altKey && e.key === "ArrowDown") {
            e.preventDefault();
            open();
          }
        }}
      />
      <button
        ref={trigger}
        type="button"
        tabIndex={props.readOnly ? -1 : 0}
        disabled={props.disabled || props.readOnly}
        aria-label={`Choose ${fieldName.replace(/^choose\s+/i, "")} from calendar`}
        aria-expanded={!!month}
        aria-haspopup="dialog"
        onClick={(e) => {
          e.preventDefault();
          open();
        }}
      >
        <NxIcon name="schedule" size={18} />
      </button>
      {month && (
        <Flyout
          role="dialog"
          label="Choose a date"
          id={id}
          anchor={trigger}
          close={() => setMonth("")}
          className="nx-date-menu"
        >
          <div className="nx-month-nav">
            <button
              type="button"
              aria-label="Previous month"
              disabled={month <= "0100-01-01"}
              onClick={() => setMonth(addMonths(month, -1))}
            >
              ‹
            </button>
            <strong>
              {new Intl.DateTimeFormat("en", {
                month: "long",
                timeZone: "UTC",
              }).format(new Date(month + "T12:00Z"))}
            </strong>
            <input
              aria-label="Calendar year"
              key={month.slice(0, 4)}
              type="number"
              min="100"
              max="9999"
              defaultValue={+month.slice(0, 4)}
              onChange={(e) => {
                if (/^\d{4}$/.test(e.target.value) && +e.target.value >= 100)
                  setMonth(e.target.value + month.slice(4));
              }}
            />
            <button
              type="button"
              aria-label="Next month"
              disabled={month >= "9999-12-01"}
              onClick={() => setMonth(addMonths(month, 1))}
            >
              ›
            </button>
          </div>
          <div
            className="nx-date-grid"
            onKeyDown={(e) => {
              const keyOffsets: Record<string, number> = {
                ArrowLeft: -1,
                ArrowRight: 1,
                ArrowUp: -7,
                ArrowDown: 7,
              };
              const current = (e.target as HTMLElement).dataset.day;
              if (!current || !(e.key in keyOffsets)) return;
              e.preventDefault();
              const next = addDays(current, keyOffsets[e.key]);
              if (!validCalendarDay(next) || !inRange(next)) return;
              setMonth(next.slice(0, 7) + "-01");
              requestAnimationFrame(() =>
                document.getElementById(id + "-" + next)?.focus(),
              );
            }}
          >
            {["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"].map((d) => (
              <small key={d}>{d}</small>
            ))}
            {monthGrid(month).map((day) => (
              <button
                id={id + "-" + day}
                data-day={day}
                key={day}
                type="button"
                style={{ gridColumnStart: weekdayIndex(day) + 1 }}
                aria-label={formatYmdLong(day)}
                aria-pressed={day === datePart}
                aria-current={day === localToday() ? "date" : undefined}
                data-outside={day.slice(0, 7) !== month.slice(0, 7)}
                disabled={!inRange(day)}
                onClick={() => choose(day)}
              >
                {+day.slice(8)}
              </button>
            ))}
          </div>
          <div className="nx-picker-foot">
            <button type="button" onClick={() => choose("")}>
              Clear
            </button>
            <button
              type="button"
              disabled={!inRange(localToday())}
              onClick={() => choose(localToday())}
            >
              Today
            </button>
          </div>
        </Flyout>
      )}
    </span>
  );
}
