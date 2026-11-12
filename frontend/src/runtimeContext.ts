/**
 * Whether the page is a developer's own copy: a development build opened on this computer.
 * Only then may a screen talk about starting the database and API. A production build, or
 * any page reached by a real hostname, is in front of staff or patients and gets plain words.
 */
export function isLocalDevelopment(developmentBuild: boolean, hostname: string): boolean {
  if (!developmentBuild) return false;
  const host = hostname.toLowerCase();
  return host === "localhost" || host.endsWith(".localhost") || host === "[::1]" || host === "::1" || /^127(\.\d{1,3}){3}$/.test(host);
}
