import assert from "node:assert/strict";
import test from "node:test";
import { isLoadFailure } from "./pageFailure.ts";
import { isLocalDevelopment } from "./runtimeContext.ts";
import { readStored, writeStored } from "./safeStorage.ts";

test("a page that could not be downloaded is recognised in each browser's wording", () => {
  for (const message of [
    "Failed to fetch dynamically imported module: https://demo.example.com/assets/App-abc123.js",
    "error loading dynamically imported module: https://demo.example.com/assets/App-abc123.js",
    "Importing a module script failed.",
    "Unable to preload CSS for /assets/App-abc123.css",
  ])
    assert.equal(isLoadFailure(new TypeError(message)), true, message);
  assert.equal(isLoadFailure(Object.assign(new Error("Loading chunk 4 failed."), { name: "ChunkLoadError" })), true);
});

test("an ordinary error while drawing is not mistaken for a failed download", () => {
  assert.equal(isLoadFailure(new TypeError("Cannot read properties of undefined (reading 'id')")), false);
  assert.equal(isLoadFailure(new Error("Failed to fetch")), false);
  assert.equal(isLoadFailure("Failed to fetch dynamically imported module"), false);
  assert.equal(isLoadFailure(null), false);
  assert.equal(isLoadFailure(undefined), false);
});

test("developer wording needs both a development build and this computer", () => {
  for (const host of ["localhost", "127.0.0.1", "127.0.0.42", "[::1]", "app.localhost", "LOCALHOST"])
    assert.equal(isLocalDevelopment(true, host), true, host);
  for (const host of ["demo.example.com", "nexus.vercel.app", "192.168.1.20", "10.0.0.5", "localhost.example.com", "127.0.0.1.example.com", ""])
    assert.equal(isLocalDevelopment(true, host), false, host);
});

test("a production build never shows developer wording, even on this computer", () => {
  for (const host of ["localhost", "127.0.0.1", "[::1]"]) assert.equal(isLocalDevelopment(false, host), false, host);
});

type Globals = { localStorage?: unknown; sessionStorage?: unknown };
function withStorage(define: (target: Globals) => void, run: () => void): void {
  const saved = {
    local: Object.getOwnPropertyDescriptor(globalThis, "localStorage"),
    session: Object.getOwnPropertyDescriptor(globalThis, "sessionStorage"),
  };
  try {
    define(globalThis as Globals);
    run();
  } finally {
    for (const [name, descriptor] of [["localStorage", saved.local], ["sessionStorage", saved.session]] as const) {
      if (descriptor) Object.defineProperty(globalThis, name, descriptor);
      else delete (globalThis as Globals)[name];
    }
  }
}
function memoryStorage() {
  const items = new Map<string, string>();
  return {
    getItem: (key: string) => items.get(key) ?? null,
    setItem: (key: string, value: string) => void items.set(key, value),
    removeItem: (key: string) => void items.delete(key),
  };
}
const install = (name: "localStorage" | "sessionStorage", descriptor: PropertyDescriptor) => (target: Globals) =>
  void Object.defineProperty(target, name, { configurable: true, ...descriptor });

test("storage that works is read, written and cleared, each kind on its own", () => {
  const local = memoryStorage();
  const session = memoryStorage();
  withStorage(
    (target) => {
      install("localStorage", { value: local })(target);
      install("sessionStorage", { value: session })(target);
    },
    () => {
      assert.equal(readStored("local", "nexus.test"), null);
      assert.equal(writeStored("local", "nexus.test", "kept"), true);
      assert.equal(readStored("local", "nexus.test"), "kept");
      assert.equal(readStored("session", "nexus.test"), null);
      assert.equal(writeStored("local", "nexus.test", null), true);
      assert.equal(readStored("local", "nexus.test"), null);
    },
  );
});

test("a browser that refuses to hand over storage reads as empty instead of throwing", () => {
  const blocked = { get() { throw new Error("The operation is insecure."); } };
  withStorage(
    (target) => {
      install("localStorage", blocked)(target);
      install("sessionStorage", blocked)(target);
    },
    () => {
      assert.equal(readStored("local", "nexus.local-operator"), null);
      assert.equal(readStored("session", "nexus.visit-access"), null);
      assert.equal(writeStored("local", "nexus.local-operator", "staff-1"), false);
      assert.equal(writeStored("session", "nexus.visit-access", null), false);
    },
  );
});

test("storage whose calls throw, as when it is full, also fails quietly", () => {
  const broken = {
    getItem: () => { throw new Error("denied"); },
    setItem: () => { throw new Error("quota exceeded"); },
    removeItem: () => { throw new Error("denied"); },
  };
  withStorage(install("localStorage", { value: broken }), () => {
    assert.equal(readStored("local", "nexus.theme"), null);
    assert.equal(writeStored("local", "nexus.theme", "dark"), false);
    assert.equal(writeStored("local", "nexus.theme", null), false);
  });
});

test("a missing storage area is treated as empty", () => {
  withStorage(install("sessionStorage", { value: undefined }), () => {
    assert.equal(readStored("session", "nexus.visit-access"), null);
    assert.equal(writeStored("session", "nexus.visit-access", "x"), false);
  });
});
