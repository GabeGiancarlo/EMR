/**
 * Browser storage that never throws.
 *
 * A browser can refuse site storage outright (blocked site data, some private windows,
 * an embedded frame). Then even naming `localStorage` throws, and a read made while
 * rendering would blank the page. These helpers answer "nothing stored" instead.
 */
export type StorageKind = "local" | "session";

function area(kind: StorageKind): Storage | null {
  try {
    return (kind === "local" ? globalThis.localStorage : globalThis.sessionStorage) ?? null;
  } catch {
    return null;
  }
}

export function readStored(kind: StorageKind, key: string): string | null {
  try {
    return area(kind)?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

/** Store a value, or remove it when `value` is null. False means the browser did not keep it. */
export function writeStored(kind: StorageKind, key: string, value: string | null): boolean {
  try {
    const storage = area(kind);
    if (!storage) return false;
    if (value === null) storage.removeItem(key);
    else storage.setItem(key, value);
    return true;
  } catch {
    return false;
  }
}
