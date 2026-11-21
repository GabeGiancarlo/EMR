import { createContext, useContext, useEffect, useState } from "react";
import { authorizedFetch } from "../api";
import type { Row } from "../api";
import { pictureFor } from "../avatars";
import type { Motif } from "../avatars";
import { nexusArt } from "../brand/art";
import portraits from "../brand/demo-portraits.jpg";

/** The chosen picture for each person the workspace returned, keyed by their id. */
const ChosenPictures = createContext<Record<string, string>>({});
export const AvatarProvider = ChosenPictures.Provider;

export type AvatarKind = "clients" | "staff";
type Size = "sm" | "md" | "lg" | "xl";

/**
 * Uploaded photos, fetched once per version as the signed-in person and held in memory
 * only. The API serves them with no-store; nothing is written to the browser's cache.
 */
const photos = new Map<string, { tag: string; url: Promise<string> }>();
function photo(kind: AvatarKind, id: string, tag: string): Promise<string> {
  const key = `${kind}/${id}`;
  const held = photos.get(key);
  if (held?.tag === tag) return held.url;
  held?.url.then((url) => URL.revokeObjectURL(url), () => undefined);
  const url = authorizedFetch(`/api/profile-pictures/${kind}/${id}`, { headers: { Accept: "image/*" } }).then(
    async (response) => {
      if (!response.ok) throw new Error("Photo unavailable");
      return URL.createObjectURL(await response.blob());
    },
  );
  url.catch(() => {
    if (photos.get(key)?.tag === tag) photos.delete(key);
  });
  photos.set(key, { tag, url });
  return url;
}
/** Drop every held photo: another person is now using this browser. */
export function forgetPhotos() {
  for (const held of photos.values()) held.url.then((url) => URL.revokeObjectURL(url), () => undefined);
  photos.clear();
}

/** The three fictional portraits belong only to the seeded demonstration roster. */
const demoRoster = ["alex", "jordan", "taylor"];
export function demoPortrait(patient: Row | undefined): number {
  return demoRoster.findIndex(
    (first) =>
      patient?.email === first + ".synthetic@example.invalid" &&
      patient.last_name === "Synthetic" &&
      patient.emergency_contact === "Synthetic demonstration record; no real contact.",
  );
}

/** One motif from the brand kit, in the colour of the text around it. */
export function MotifMark({ motif }: { motif: Motif }) {
  return (
    <svg viewBox={nexusArt.motifViewBox} aria-hidden="true">
      <path fill="currentColor" fillRule="evenodd" d={nexusArt.motifs[motif]} />
    </svg>
  );
}

/**
 * A person's picture as a Nexus tile: an uploaded photo, a chosen motif, or the automatic
 * default. It is decorative, because the name is always beside it; a control that only
 * shows the picture must carry its own label.
 *
 * `choice` overrides the workspace's value, for previewing a picture before it is saved.
 */
export function Avatar({
  id,
  kind = "clients",
  size = "md",
  patient,
  choice,
  preview,
  className = "",
}: {
  id: string;
  kind?: AvatarKind;
  size?: Size;
  /** The client row, when there is one: the demonstration roster keeps its portraits. */
  patient?: Row;
  choice?: string | null;
  /** A local image to show instead, such as a photo that has not been saved yet. */
  preview?: string;
  /** Kept so existing callers need not change; the name is shown beside the tile. */
  name?: string;
  className?: string;
}) {
  const chosen = useContext(ChosenPictures)[id];
  const selected = choice === undefined ? chosen : choice;
  const picture = pictureFor(id, selected);
  const tag = picture.kind === "photo" ? picture.tag : "";
  const [held, setHeld] = useState<{ tag: string; url: string } | null>(null);
  useEffect(() => {
    if (!tag) return;
    let live = true;
    photo(kind, id, tag).then(
      (url) => live && setHeld({ tag, url }),
      () => live && setHeld(null),
    );
    return () => {
      live = false;
    };
  }, [kind, id, tag]);
  const image = preview || (tag && held?.tag === tag ? held.url : "");
  const slot = !image && !selected ? demoPortrait(patient) : -1;
  return (
    <span
      aria-hidden="true"
      className={`nx-avatar nx-avatar--${size} tone-${picture.tone} ${className}`.trim()}
      data-picture={image || slot >= 0 ? "photo" : "motif"}
    >
      {image ? (
        <img src={image} alt="" />
      ) : slot >= 0 ? (
        <span
          className="nx-avatar-portrait"
          style={{ backgroundImage: `url(${portraits})`, backgroundSize: "300% 100%", backgroundPosition: `${slot * 50}% center` }}
        />
      ) : (
        <MotifMark motif={picture.motif} />
      )}
    </span>
  );
}
