/** Where the API lives. Empty means same origin, which is how both the Vite proxy and the hosted rewrite serve it. */
const base =
  (import.meta.env.VITE_API_BASE_URL as string | undefined)?.replace(
    /\/$/,
    "",
  ) ?? "";
export const apiUrl = (path: string): string => `${base}${path}`;
