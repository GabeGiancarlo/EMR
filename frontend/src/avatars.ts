/**
 * Which picture stands for a person.
 *
 * The workspace sends a chosen picture per person: "photo:<tag>" for an uploaded photo, or
 * the name of a Nexus motif picked as a default. Someone with nothing chosen gets an
 * automatic default: a motif and a colour worked out from their id, so the same person
 * always looks the same and a list of people never shows one repeated mark.
 */
export const MOTIFS = ["sprig", "fern", "sprout", "pine", "hill", "river", "cairn", "slab", "sun", "moon"] as const;
export type Motif = (typeof MOTIFS)[number];

/** How the motifs are named where someone chooses one. */
export const MOTIF_NAMES: Record<Motif, string> = {
  sprig: "Sprig",
  fern: "Fern",
  sprout: "Sprout",
  pine: "Pine",
  hill: "Hills",
  river: "River",
  cairn: "Cairn",
  slab: "Tree rings",
  sun: "Sun",
  moon: "Moon",
};

/** Colour pairings for a default picture. Each is a pair of role tokens in the stylesheet. */
export const TONES = 6;

/** FNV-1a: small, stable, and spreads ids that differ by one character. */
function hash(text: string): number {
  let value = 0x811c9dc5;
  for (let index = 0; index < text.length; index++) {
    value ^= text.charCodeAt(index);
    value = Math.imul(value, 0x01000193);
  }
  return value >>> 0;
}

export function automaticPicture(id: string): { motif: Motif; tone: number } {
  const value = hash(id);
  return { motif: MOTIFS[value % MOTIFS.length], tone: Math.floor(value / MOTIFS.length) % TONES };
}

export type Picture =
  | { kind: "photo"; tag: string; motif: Motif; tone: number }
  | { kind: "motif"; motif: Motif; tone: number; chosen: boolean };

/** The picture to draw. A photo keeps its automatic default to show while it loads or if it cannot. */
export function pictureFor(id: string, choice: string | undefined | null): Picture {
  const automatic = automaticPicture(id);
  if (choice && /^photo:[0-9a-f]{16}$/.test(choice)) return { kind: "photo", tag: choice.slice(6), ...automatic };
  if (choice && (MOTIFS as readonly string[]).includes(choice))
    return { kind: "motif", motif: choice as Motif, tone: automatic.tone, chosen: true };
  return { kind: "motif", ...automatic, chosen: false };
}

/** The largest centred square in an image, as a source rectangle for drawing. */
export function squareCrop(width: number, height: number): { x: number; y: number; side: number } {
  const side = Math.max(0, Math.min(width, height));
  return { x: Math.floor((width - side) / 2), y: Math.floor((height - side) / 2), side };
}

export const PHOTO_TYPES = ["image/jpeg", "image/png", "image/webp"];
/** The side, in pixels, of the square photo the app stores. */
export const PHOTO_SIDE = 256;
