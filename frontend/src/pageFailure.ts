/**
 * Tell a page that could not be downloaded from a page that broke while drawing.
 *
 * The workspace and the visit page are separate downloads. When one fails to arrive (a
 * dropped connection, or a release that replaced the file while the tab was open) the
 * browser reports it in its own words; these are the phrasings of Chromium, Firefox,
 * Safari and the bundler's stylesheet preloader.
 */
const LOAD_FAILURE = /dynamically imported module|importing a module script failed|unable to preload css|chunkloaderror|loading (css )?chunk/i;

export function isLoadFailure(error: unknown): boolean {
  if (typeof error !== "object" || error === null) return false;
  const { name, message } = error as { name?: unknown; message?: unknown };
  return LOAD_FAILURE.test(`${typeof name === "string" ? name : ""} ${typeof message === "string" ? message : ""}`);
}
