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
import { formatDateText, formatTimeText, parseDateText, parseTimeText } from "../dateText";

type Anchor = RefObject<HTMLElement | null>;

/** The nearest ancestor that scrolls vertically, so a menu can ask it for room. */
function scrollParent(node: HTMLElement): HTMLElement | null {
  for (let parent = node.parentElement; parent; parent = parent.parentElement) {
    const overflow = getComputedStyle(parent).overflowY;
    if ((overflow === "auto" || overflow === "scroll") && parent.scrollHeight > parent.clientHeight + 1) return parent;
  }
  return null;
}

/**
 * A menu in the top layer, tied to the control that opened it. It opens below its control,
 * scrolling the form for room first; when it has to open above, it names its field so nobody
 * mistakes it for the one it covers.
 */
function Flyout({
  anchor,
  close,
  children,
  id,
  role,
  label,
  className,
  limit = 336,
  returnFocus,
}: {
  anchor: Anchor;
  /** Where focus goes on Escape when the anchor is a wrapper that cannot take it. */
  returnFocus?: Anchor;
  close: () => void;
  children: ReactNode;
  id: string;
  role: "dialog" | "listbox";
  label: string;
  className: string;
  limit?: number;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const latestClose = useRef(close);
  latestClose.current = close;
  useLayoutEffect(() => {
    const menu = panel.current;
    const control = anchor.current;
    if (!menu || !control) return;
    menu.showPopover?.();
    let first = true;
    function place() {
      if (!menu || !control) return;
      let r = control.getBoundingClientRect();
      const width = Math.min(
        window.innerWidth - 16,
        Math.max(r.width, role === "dialog" ? 308 : 208),
      );
      menu.style.width = width + "px";
      const scrolled = menu.scrollTop;
      menu.style.maxHeight = "";
      const want = Math.min(menu.scrollHeight + 2, limit);
      let below = window.innerHeight - r.bottom - 12;
      if (first && below < want) {
        const scroller = scrollParent(control);
        if (scroller) {
          const spare = scroller.scrollHeight - scroller.clientHeight - scroller.scrollTop;
          const keepVisible = r.top - scroller.getBoundingClientRect().top - 44;
          const move = Math.min(want - below, spare, keepVisible);
          if (move > 0) {
            scroller.scrollTop += move;
            r = control.getBoundingClientRect();
            below = window.innerHeight - r.bottom - 12;
          }
        }
      }
      first = false;
      const above = r.top - 12;
      // A list can scroll inside a short space. A calendar cannot lose its last rows, so it
      // goes above whenever it does not fit below and there is more room there.
      const flip = (role === "dialog" ? below < want : below < Math.min(want, 190)) && above > below;
      menu.dataset.side = flip ? "above" : "below";
      menu.style.maxHeight = Math.max(120, Math.min(limit, flip ? above : below)) + "px";
      menu.style.left =
        Math.max(8, Math.min(r.left, window.innerWidth - width - 8)) + "px";
      menu.style.top =
        (flip ? Math.max(6, r.top - menu.offsetHeight - 4) : r.bottom + 4) + "px";
      menu.scrollTop = scrolled;
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
        (returnFocus?.current ?? control!).focus();
      }
    }
    // The page scrolling moves the control, so the menu follows. The menu's own list scrolling
    // must not: measuring again would send the list back to its top.
    function follow(e: Event) {
      if (!(e.target instanceof Node) || !menu!.contains(e.target)) place();
    }
    window.addEventListener("resize", place);
    window.addEventListener("scroll", follow, true);
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("focusin", outside);
    document.addEventListener("keydown", escape, true);
    return () => {
      window.removeEventListener("resize", place);
      window.removeEventListener("scroll", follow, true);
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("focusin", outside);
      document.removeEventListener("keydown", escape, true);
    };
  }, [anchor, role, limit]);
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
      <div className="nx-flyout-caption" aria-hidden="true">{label}</div>
      {children}
    </div>
  );
}

/** The words of the label that names a control, without its required mark or nested controls. */
function labelText(control: HTMLInputElement | HTMLSelectElement | null, fallback: string) {
  const label = control?.labels?.[0]?.cloneNode(true) as HTMLElement | undefined;
  label?.querySelectorAll("input,button,select,small,[popover],.nx-choice,.nx-date,.nx-time").forEach((node) => node.remove());
  return label?.textContent?.replace(/\s*\*\s*$/, "").replace(/\s+/g, " ").trim() || fallback;
}

type Option = { value: string; text: string; disabled: boolean; placeholder: boolean };
/**
 * A select with the Nexus menu. The native element stays in the form, so labels, required
 * checks and change handlers work as written; only the list is drawn here.
 */
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
        // "Choose…" is an instruction, not a choice: a required field cannot be set back to it.
        placeholder: o.value === "" && select.required,
      })),
    );
    setActive(select.options[select.selectedIndex]?.value === "" && select.required ? -1 : select.selectedIndex);
  }
  function choose(index: number) {
    const select = ref.current;
    if (
      !select ||
      !options?.[index] ||
      options[index].disabled ||
      options[index].placeholder ||
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
    props.onKeyDown?.(e);
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
      if (active >= 0) choose(active);
      return;
    }
    const enabled = options
      .map((o, i) => (o.disabled || o.placeholder ? -1 : i))
      .filter((i) => i >= 0);
    if (!enabled.length) return;
    e.preventDefault();
    if (e.key === "Home") setActive(enabled[0]);
    else if (e.key === "End") setActive(enabled[enabled.length - 1]);
    else if (["ArrowUp", "ArrowDown"].includes(e.key)) {
      const at = enabled.indexOf(active);
      const i = at < 0 ? (e.key === "ArrowUp" ? enabled.length - 1 : 0) : at + (e.key === "ArrowUp" ? -1 : 1);
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
  const current = ref.current?.value;
  return (
    <span className="nx-choice" data-open={options ? "" : undefined}>
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
      <NxIcon name={options ? "chevron-up" : "chevron-down"} size={15} />
      {options && (
        <Flyout
          id={id}
          anchor={ref}
          close={() => setOptions(null)}
          role="listbox"
          label={props["aria-label"] || labelText(ref.current, "Choices")}
          className="nx-options"
        >
          {options.map((o, i) =>
            o.placeholder ? null : (
              <div
                id={id + "-" + i}
                key={o.value + "-" + i}
                role="option"
                aria-selected={current === o.value}
                aria-disabled={o.disabled}
                data-active={i === active}
                data-empty={o.value === "" ? "" : undefined}
                onPointerMove={() => !o.disabled && setActive(i)}
                onMouseDown={(e) => e.preventDefault()}
                onClick={(e) => {
                  e.preventDefault();
                  choose(i);
                }}
              >
                <span>{o.text}</span>
                {current === o.value && <NxIcon name="check" size={15} />}
              </div>
            ),
          )}
          {options.every((o) => o.placeholder) && (
            <div className="nx-options-empty">Nothing to choose from yet.</div>
          )}
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

/** A date and a time kept together as YYYY-MM-DDTHH:MM, shown as "Nov 18, 2026" and "3:00 PM". */
export function DateTimeField({
  id,
  label,
  value,
  onChange,
  required,
  disabled,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (next: string) => void;
  required?: boolean;
  disabled?: boolean;
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
        disabled={disabled}
        value={day}
        onChange={(e) => change(e.target.value, time)}
      />
      <TimeControl
        label={label + " time"}
        value={time}
        required={required || !!day}
        disabled={disabled}
        onChange={(next) => change(day, next)}
      />
    </span>
  );
}

/**
 * A time, typed the way people say it ("3pm", "9:30", "1500") and shown as "3:00 PM".
 * `value` and `onChange` use the stored 24-hour "HH:MM".
 */
export function TimeControl({
  id,
  label,
  value,
  onChange,
  required,
  disabled,
}: {
  id?: string;
  label: string;
  value: string;
  onChange: (next: string) => void;
  required?: boolean;
  disabled?: boolean;
}) {
  const field = useRef<HTMLSpanElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menuId = useId();
  const [text, setText] = useState(() => formatTimeText(value));
  const [editing, setEditing] = useState(false);
  const [open, setOpen] = useState(false);
  useEffect(() => {
    // Typed text that is not a time stays as typed, so it can be corrected.
    if (!editing && (value || !text.trim() || parseTimeText(text))) setText(formatTimeText(value));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value, editing]);
  const invalid = text.trim() !== "" && !parseTimeText(text);
  useLayoutEffect(() => {
    input.current?.setCustomValidity(invalid ? "Enter a time like 3:00 PM." : "");
  }, [invalid]);
  // The listed quarter hour nearest the current value: where the list opens and takes focus.
  const [hourNow, minuteNow] = (value || "09:00").split(":").map(Number);
  const nearestSlot = Math.min(95, Math.round((hourNow * 60 + minuteNow) / 15));
  const nearest = `${String(Math.floor(nearestSlot / 4)).padStart(2, "0")}:${String((nearestSlot % 4) * 15).padStart(2, "0")}`;
  useEffect(() => {
    if (!open) return;
    const selected = document.getElementById(menuId + "-" + nearest);
    selected?.scrollIntoView({ block: "center" });
    selected?.focus();
    // Only when the list opens.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);
  function type(next: string) {
    setText(next);
    const parsed = parseTimeText(next);
    const stored = next.trim() === "" ? "" : parsed ?? "";
    if (stored !== value) onChange(stored);
  }
  function choose(next: string) {
    setText(formatTimeText(next));
    if (next !== value) onChange(next);
    setOpen(false);
    input.current?.focus();
  }
  function step(minutes: number) {
    const [hour, minute] = (value || "09:00").split(":").map(Number);
    const total = (hour * 60 + minute + minutes + 1440) % 1440;
    const snapped = Math.round(total / 15) * 15 % 1440;
    const next = `${String(Math.floor(snapped / 60)).padStart(2, "0")}:${String(snapped % 60).padStart(2, "0")}`;
    setText(formatTimeText(next));
    onChange(next);
  }
  const times = Array.from({ length: 96 }, (_, i) =>
    `${String(Math.floor(i / 4)).padStart(2, "0")}:${String((i % 4) * 15).padStart(2, "0")}`);
  return (
    <span ref={field} className="nx-time" data-invalid={invalid && !editing ? "" : undefined} data-open={open ? "" : undefined}>
      <input
        ref={input}
        id={id}
        aria-label={label}
        aria-invalid={invalid && !editing ? true : undefined}
        value={text}
        required={required}
        disabled={disabled}
        placeholder="3:00 PM"
        aria-keyshortcuts="Alt+ArrowDown"
        autoComplete="off"
        maxLength={12}
        onFocus={() => setEditing(true)}
        onBlur={() => setEditing(false)}
        onChange={(e) => type(e.target.value)}
        onKeyDown={(e) => {
          if (e.altKey && e.key === "ArrowDown") {
            e.preventDefault();
            setOpen(true);
          } else if (!e.altKey && (e.key === "ArrowDown" || e.key === "ArrowUp") && !invalid) {
            e.preventDefault();
            step(e.key === "ArrowUp" ? 15 : -15);
          }
        }}
      />
      <button
        ref={trigger}
        type="button"
        tabIndex={-1}
        disabled={disabled}
        title="Choose a time (Alt+Down)"
        aria-label={`Choose ${label.toLowerCase()} from a list`}
        aria-expanded={open}
        aria-haspopup="listbox"
        onClick={(e) => {
          e.preventDefault();
          setOpen((was) => !was);
        }}
      >
        <NxIcon name="clock" size={17} />
      </button>
      {invalid && !editing && <span className="nx-field-error" role="alert"><NxIcon name="error" size={14} />Enter a time like 3:00 PM.</span>}
      {open && (
        <Flyout
          id={menuId}
          role="listbox"
          label={label}
          anchor={field}
          returnFocus={trigger}
          close={() => setOpen(false)}
          className="nx-options nx-time-menu"
          limit={264}
        >
          <div
            onKeyDown={(e) => {
              if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
              e.preventDefault();
              const at = times.indexOf((e.target as HTMLElement).dataset.time || "");
              const next = times[(at + (e.key === "ArrowDown" ? 1 : -1) + times.length) % times.length];
              document.getElementById(menuId + "-" + next)?.focus();
            }}
          >
            {times.map((time) => (
              <button
                type="button"
                role="option"
                key={time}
                id={menuId + "-" + time}
                data-time={time}
                aria-selected={time === value}
                tabIndex={time === nearest ? 0 : -1}
                onClick={() => choose(time)}
              >
                <span>{formatTimeText(time)}</span>
                {time === value && <NxIcon name="check" size={15} />}
              </button>
            ))}
          </div>
        </Flyout>
      )}
    </span>
  );
}

/**
 * A date field. The record keeps YYYY-MM-DD, carried by a hidden input that fires the
 * caller's ordinary change handlers; the visible field shows "Nov 18, 2026" and takes typed
 * dates in the usual forms. The calendar marks the chosen day with a leaf.
 */
function DateControl({
  min,
  max,
  value,
  onChange,
  onInput,
  maxLength: _storedLength,
  required,
  disabled,
  readOnly,
  id,
  name,
  className,
  placeholder,
  ...props
}: InputProps) {
  const field = useRef<HTMLSpanElement>(null);
  const stored = useRef<HTMLInputElement>(null);
  const shown = useRef<HTMLInputElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const menuId = useId();
  const iso = String(value ?? "").slice(0, 10);
  const [text, setText] = useState(() => formatDateText(iso) || iso);
  const [editing, setEditing] = useState(false);
  const [month, setMonth] = useState("");
  const [fieldName, setFieldName] = useState("date");
  const lower = String(min ?? "").slice(0, 10);
  const upper = String(max ?? "").slice(0, 10);
  const inRange = (day: string) => (!lower || day >= lower) && (!upper || day <= upper);
  const parsed = text.trim() ? parseDateText(text, localToday()) : null;
  const problem = !text.trim()
    ? ""
    : !parsed
      ? "Enter a date like 11/18/2026 or Nov 18, 2026."
      : lower && parsed < lower
        ? `Choose ${formatDateText(lower)} or later.`
        : upper && parsed > upper
          ? `Choose ${formatDateText(upper)} or earlier.`
          : "";
  useLayoutEffect(() => {
    setFieldName(props["aria-label"] || labelText(shown.current, "date"));
  }, [id, props["aria-label"]]);
  useLayoutEffect(() => {
    shown.current?.setCustomValidity(problem);
  }, [problem]);
  useEffect(() => {
    // Follow the record when it changes from outside, but never rewrite text someone is typing
    // or text that still needs correcting.
    if (editing) return;
    if (iso) setText(formatDateText(iso) || iso);
    else if (!text.trim() || !problem) setText("");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [iso, editing]);
  function emit(next: string) {
    const input = stored.current;
    if (!input || input.value === next) return;
    // Through the real input, so the form's change, validation and unsaved-work handlers run.
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(input, next);
    input.dispatchEvent(new Event("input", { bubbles: true }));
  }
  function type(next: string) {
    setText(next);
    const day = next.trim() ? parseDateText(next, localToday()) : null;
    emit(day && inRange(day) ? day : "");
  }
  function toggle() {
    if (month) {
      setMonth("");
      return;
    }
    let day = validCalendarDay(iso) ? iso : localToday();
    if (upper && day > upper) day = upper;
    if (lower && day < lower) day = lower;
    setMonth(day.slice(0, 7) + "-01");
  }
  function choose(day: string) {
    setText(formatDateText(day));
    emit(day);
    setMonth("");
    shown.current?.focus();
  }
  const today = localToday();
  const days = month ? monthGrid(month) : [];
  const inMonth = (day: string) => day.slice(0, 7) === month.slice(0, 7);
  const entry = month
    ? (iso && inMonth(iso) && inRange(iso) ? iso : inMonth(today) && inRange(today) ? today : days.find((day) => inMonth(day) && inRange(day))) || ""
    : "";
  useEffect(() => {
    if (month) requestAnimationFrame(() => document.getElementById(menuId + "-" + entry)?.focus());
    // Only when the calendar opens: arrow keys move focus themselves afterwards.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [!!month]);
  const showProblem = !!problem && !editing;
  return (
    <span ref={field} className={`nx-date ${className || ""}`.trim()} data-invalid={showProblem ? "" : undefined} data-open={month ? "" : undefined}>
      <input
        ref={stored}
        hidden
        readOnly
        tabIndex={-1}
        aria-hidden="true"
        name={name}
        value={iso}
        onChange={onChange}
        onInput={onInput}
      />
      <input
        {...props}
        ref={shown}
        id={id}
        type="text"
        value={text}
        required={required}
        disabled={disabled}
        readOnly={readOnly}
        autoComplete={props.autoComplete ?? "off"}
        placeholder={placeholder ?? "MM/DD/YYYY"}
        aria-keyshortcuts="Alt+ArrowDown"
        aria-invalid={showProblem ? true : undefined}
        maxLength={24}
        onFocus={(e) => {
          setEditing(true);
          props.onFocus?.(e);
        }}
        onBlur={(e) => {
          setEditing(false);
          props.onBlur?.(e);
        }}
        onChange={(e) => type(e.target.value)}
        onKeyDown={(e) => {
          props.onKeyDown?.(e);
          if (e.altKey && e.key === "ArrowDown") {
            e.preventDefault();
            toggle();
          }
        }}
      />
      <button
        ref={trigger}
        type="button"
        // Out of the tab order so Tab goes field to field; Alt+Down opens the calendar from the field.
        tabIndex={-1}
        title="Choose from calendar (Alt+Down)"
        disabled={disabled || readOnly}
        aria-label={`Choose ${fieldName.replace(/^choose\s+/i, "")} from calendar`}
        aria-expanded={!!month}
        aria-haspopup="dialog"
        onClick={(e) => {
          e.preventDefault();
          toggle();
        }}
      >
        <NxIcon name="schedule" size={17} />
      </button>
      {showProblem && <span className="nx-field-error" role="alert"><NxIcon name="error" size={14} />{problem}</span>}
      {month && (
        <Flyout
          role="dialog"
          label={fieldName.charAt(0).toUpperCase() + fieldName.slice(1)}
          id={menuId}
          anchor={field}
          returnFocus={trigger}
          close={() => setMonth("")}
          className="nx-date-menu"
          limit={420}
        >
          <div className="nx-month-nav">
            <button
              type="button"
              aria-label="Previous month"
              disabled={month <= "0100-01-01"}
              onClick={() => setMonth(addMonths(month, -1))}
            >
              <NxIcon name="chevron-left" size={17} />
            </button>
            <strong aria-live="polite">
              {new Intl.DateTimeFormat("en-US", {
                month: "long",
                timeZone: "UTC",
              }).format(new Date(month + "T12:00Z"))}
            </strong>
            <input
              aria-label="Year"
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
              <NxIcon name="chevron-right" size={17} />
            </button>
          </div>
          <div
            className="nx-date-grid"
            onKeyDown={(e) => {
              const current = (e.target as HTMLElement).dataset.day;
              if (!current) return;
              const weekday = weekdayIndex(current);
              const next =
                e.key === "ArrowLeft" ? addDays(current, -1)
                : e.key === "ArrowRight" ? addDays(current, 1)
                : e.key === "ArrowUp" ? addDays(current, -7)
                : e.key === "ArrowDown" ? addDays(current, 7)
                : e.key === "Home" ? addDays(current, -weekday)
                : e.key === "End" ? addDays(current, 6 - weekday)
                : e.key === "PageUp" ? addMonths(current, -1)
                : e.key === "PageDown" ? addMonths(current, 1)
                : "";
              if (!next) return;
              e.preventDefault();
              if (!validCalendarDay(next) || !inRange(next)) return;
              setMonth(next.slice(0, 7) + "-01");
              requestAnimationFrame(() =>
                document.getElementById(menuId + "-" + next)?.focus(),
              );
            }}
          >
            {["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"].map((d) => (
              <small key={d}>{d}</small>
            ))}
            {days.map((day) => (
              <button
                id={menuId + "-" + day}
                data-day={day}
                key={day}
                type="button"
                tabIndex={day === entry ? 0 : -1}
                style={{ gridColumnStart: weekdayIndex(day) + 1 }}
                aria-label={formatYmdLong(day) + (day === today ? ", today" : "")}
                aria-pressed={day === iso}
                aria-current={day === today ? "date" : undefined}
                data-outside={!inMonth(day)}
                disabled={!inRange(day)}
                onClick={() => choose(day)}
              >
                {+day.slice(8)}
              </button>
            ))}
          </div>
          <div className="nx-picker-foot">
            {required ? <span /> : (
              <button type="button" onClick={() => choose("")}>
                Clear
              </button>
            )}
            <button
              type="button"
              className="nx-picker-today"
              disabled={!inRange(today)}
              onClick={() => choose(today)}
            >
              <NxIcon name="leaf" size={15} />
              Today
            </button>
          </div>
        </Flyout>
      )}
    </span>
  );
}
