/**
 * How a request to the API fails, and the one rule for repeating it after sign-in recovery.
 * No browser or build globals here, so the rules can be tested on their own.
 */

/** A refused request. `status` and `code` let a screen tell "sign in again" from "not allowed". */
export class ApiRequestError extends Error {
  readonly status: number;
  readonly code: string;
  constructor(message: string, status: number, code: string) {
    super(message);
    this.name = "ApiRequestError";
    this.status = status;
    this.code = code;
  }
}

export const UNREACHABLE_MESSAGE = "Nexus could not be reached. Check your connection and try again.";
export const TIMEOUT_MESSAGE = "Nexus took too long to answer. Check your connection and try again.";

const errorName = (error: unknown): string =>
  typeof error === "object" && error !== null && "name" in error ? String((error as { name: unknown }).name) : "";

/**
 * What to throw when `fetch` itself rejected, so no screen shows the browser's own wording
 * ("Failed to fetch", "Load failed").
 *
 * A request the caller cancelled is passed through untouched: callers test for AbortError
 * to drop a result they no longer want. A timeout and every other rejection mean the API
 * was not reached; they carry status 0 and code "timeout" or "network".
 */
export function requestFailure(error: unknown): unknown {
  if (error instanceof ApiRequestError) return error;
  const name = errorName(error);
  if (name === "AbortError") return error;
  if (name === "TimeoutError") return new ApiRequestError(TIMEOUT_MESSAGE, 0, "timeout");
  return new ApiRequestError(UNREACHABLE_MESSAGE, 0, "network");
}

/** True when the API never answered, or answered that it is unwell, rather than refusing the request. */
export function isUnreachable(error: unknown): boolean {
  return !(error instanceof ApiRequestError) || error.status === 0 || error.status >= 500;
}

/**
 * Send a request; if the API answers 401 and the session can be recovered, send it once more.
 * 401 comes from the sign-in gate, before any handler ran, so repeating is safe.
 */
export async function sendWithRecovery<R extends { status: number }>(
  send: () => Promise<R>,
  recover: () => Promise<boolean>,
): Promise<R> {
  const attempt = async (): Promise<R> => {
    try {
      return await send();
    } catch (error) {
      throw requestFailure(error);
    }
  };
  const response = await attempt();
  return response.status === 401 && (await recover()) ? attempt() : response;
}
