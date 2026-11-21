import { useEffect, useRef, useState } from "react";
import type { DragEvent } from "react";
import { authorizedFetch } from "../api";
import type { Row } from "../api";
import { MOTIFS, MOTIF_NAMES, PHOTO_SIDE, PHOTO_TYPES, pictureFor, squareCrop } from "../avatars";
import type { Motif } from "../avatars";
import { Button, Modal } from "../ui";
import { Avatar, demoPortrait } from "./Avatar";
import type { AvatarKind } from "./Avatar";
import { NxIcon } from "./Icon";

type Selection =
  | { type: "keep" }
  | { type: "photo"; blob: Blob; url: string }
  | { type: "motif"; motif: Motif }
  | { type: "automatic" };

/**
 * Crop a chosen file to a centred square and keep a small JPEG copy. Redrawing the image
 * also leaves behind anything stored in the file besides the picture, such as where and
 * when it was taken.
 */
async function prepare(file: File): Promise<Blob> {
  if (!PHOTO_TYPES.includes(file.type)) throw new Error("Use a JPEG, PNG or WebP photo.");
  let bitmap: ImageBitmap;
  try {
    bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
  } catch {
    throw new Error("That file could not be read as a photo. Try another one.");
  }
  const { x, y, side } = squareCrop(bitmap.width, bitmap.height);
  if (side < 64) {
    bitmap.close();
    throw new Error("That photo is too small. Use one at least 64 pixels on each side.");
  }
  const canvas = document.createElement("canvas");
  canvas.width = canvas.height = PHOTO_SIDE;
  const context = canvas.getContext("2d");
  if (!context) throw new Error("This browser could not prepare the photo. Try another browser.");
  // A transparent picture gets the page colour behind it, not black.
  context.fillStyle = "#F4EFE3";
  context.fillRect(0, 0, PHOTO_SIDE, PHOTO_SIDE);
  context.imageSmoothingQuality = "high";
  context.drawImage(bitmap, x, y, side, side, 0, 0, PHOTO_SIDE, PHOTO_SIDE);
  bitmap.close();
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.86));
  if (!blob) throw new Error("That photo could not be prepared. Try another one.");
  return blob;
}

/**
 * Choose a person's picture: upload a photo, pick one of the Nexus defaults, or go back
 * to the automatic one. Nothing changes until Save.
 */
export function PhotoPicker({
  kind,
  id,
  name,
  current,
  patient,
  own = false,
  onClose,
  onSaved,
}: {
  kind: AvatarKind;
  id: string;
  name: string;
  /** The person's chosen picture as the workspace has it, or undefined for the automatic one. */
  current: string | undefined;
  patient?: Row;
  /** The picture is the signed-in person's own. */
  own?: boolean;
  onClose: () => void;
  onSaved: () => void | Promise<void>;
}) {
  const [selection, setSelection] = useState<Selection>({ type: "keep" });
  const [busy, setBusy] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState("");
  const [over, setOver] = useState(false);
  const file = useRef<HTMLInputElement>(null);
  const held = useRef("");
  useEffect(() => () => {
    if (held.current) URL.revokeObjectURL(held.current);
  }, []);

  const existing = pictureFor(id, current);
  const automatic = pictureFor(id, undefined);
  // What Save would store, compared with what is stored now.
  const picked =
    selection.type === "motif" ? selection.motif
    : selection.type === "automatic" ? ""
    : selection.type === "photo" ? "new photo"
    : current ?? "";
  const changed = selection.type !== "keep" && picked !== (current ?? "");
  const whose = own ? "Your" : kind === "clients" ? `${name}'s` : `${name}'s`;
  const status =
    selection.type === "photo" ? "New photo, not saved yet"
    : selection.type === "motif" ? `${MOTIF_NAMES[selection.motif]}, a Nexus default`
    : selection.type === "automatic" ? "Automatic default"
    : existing.kind === "photo" ? "Photo on file"
    : existing.chosen ? `${MOTIF_NAMES[existing.motif]}, a Nexus default`
    : demoPortrait(patient) >= 0 ? "Demonstration portrait from the seeded data"
    : "Automatic default";

  async function take(chosen: File | undefined | null) {
    if (!chosen) return;
    setError("");
    setWorking(true);
    try {
      const blob = await prepare(chosen);
      if (held.current) URL.revokeObjectURL(held.current);
      held.current = URL.createObjectURL(blob);
      setSelection({ type: "photo", blob, url: held.current });
    } catch (problem) {
      setError(problem instanceof Error ? problem.message : "That photo could not be used.");
    } finally {
      setWorking(false);
      if (file.current) file.current.value = "";
    }
  }
  function drop(event: DragEvent) {
    event.preventDefault();
    setOver(false);
    void take(event.dataTransfer.files[0]);
  }
  async function save() {
    if (!changed || busy) return;
    setBusy(true);
    setError("");
    try {
      const response = await authorizedFetch(`/api/profile-pictures/${kind}/${id}`, selection.type === "photo"
        ? { method: "PUT", headers: { "Content-Type": "image/jpeg", Accept: "application/json" }, body: selection.blob }
        : { method: "PUT", headers: { "Content-Type": "application/json", Accept: "application/json" }, body: JSON.stringify({ motif: selection.type === "motif" ? selection.motif : null }) });
      const reply = await response.json().catch(() => null);
      if (!response.ok) throw new Error(reply?.error?.message || "That did not save. Your choice is still here. Try again.");
      await onSaved();
    } catch (problem) {
      setError(problem instanceof Error && problem.message ? problem.message : "That did not save. Your choice is still here. Try again.");
      setBusy(false);
    }
  }

  const previewChoice =
    selection.type === "motif" ? selection.motif : selection.type === "automatic" ? null : selection.type === "photo" ? null : undefined;
  return (
    <Modal
      icon="camera"
      title={own ? "Your picture" : `Picture for ${name}`}
      description={kind === "clients"
        ? "A picture helps staff recognise the right person. It is not part of the clinical record."
        : "Shown beside this name across the workspace."}
      onClose={() => { if (!busy) onClose(); }}
    >
      <div className="form-body photo-picker">
        <div className="photo-current">
          <Avatar
            kind={kind}
            id={id}
            size="xl"
            patient={selection.type === "keep" ? patient : undefined}
            choice={previewChoice}
            preview={selection.type === "photo" ? selection.url : undefined}
          />
          <div>
            <strong>{name}</strong>
            <span role="status">{status}</span>
          </div>
        </div>

        <fieldset className="record-section" disabled={busy}>
          <legend><span className="nx-step" aria-hidden="true">1</span>Use a photo</legend>
          <div
            className="photo-drop"
            data-over={over ? "" : undefined}
            onDragOver={(event) => { event.preventDefault(); setOver(true); }}
            onDragLeave={() => setOver(false)}
            onDrop={drop}
          >
            <NxIcon name="upload" size={22} />
            <div>
              <strong>{working ? "Preparing the photo" : "Drop a photo here, or choose one"}</strong>
              <small>JPEG, PNG or WebP. Nexus crops it to a square and keeps a small copy. {kind === "clients" ? "Use one the client has agreed to." : ""}</small>
            </div>
            <Button variant="secondary" disabled={working || busy} onClick={() => file.current?.click()}>
              <NxIcon name="camera" size={16} />Choose a photo
            </Button>
            <input
              ref={file}
              type="file"
              hidden
              accept={PHOTO_TYPES.join(",")}
              aria-label={`Choose a photo for ${own ? "your picture" : name}`}
              onChange={(event) => void take(event.target.files?.[0])}
            />
          </div>
        </fieldset>

        <fieldset className="record-section" disabled={busy}>
          <legend><span className="nx-step" aria-hidden="true">2</span>Or pick a Nexus default</legend>
          <div className="photo-defaults" role="radiogroup" aria-label="Nexus default pictures">
            <button
              type="button"
              role="radio"
              className="photo-default"
              aria-checked={selection.type === "automatic" || (selection.type === "keep" && existing.kind === "motif" && !existing.chosen)}
              onClick={() => setSelection({ type: "automatic" })}
            >
              <Avatar kind={kind} id={id} size="lg" choice={null} />
              <span>Automatic</span>
            </button>
            {MOTIFS.map((motif) => (
              <button
                type="button"
                role="radio"
                key={motif}
                className="photo-default"
                aria-checked={selection.type === "motif" ? selection.motif === motif : selection.type === "keep" && existing.kind === "motif" && existing.chosen && existing.motif === motif}
                onClick={() => setSelection({ type: "motif", motif })}
              >
                <Avatar kind={kind} id={id} size="lg" choice={motif} />
                <span>{MOTIF_NAMES[motif]}</span>
              </button>
            ))}
          </div>
          <small className="photo-note">Automatic gives {own ? "you" : "each person"} a default of {own ? "your" : "their"} own, so a list never shows one repeated mark. {whose} is the {MOTIF_NAMES[automatic.motif].toLowerCase()}.</small>
        </fieldset>

        {error && (
          <div className="error-banner" role="alert">
            <NxIcon name="error" size={17} />
            <span>{error}</span>
          </div>
        )}
      </div>
      <div className="modal-foot">
        <span className="form-assurance">
          <NxIcon name="shield" size={15} /> Changes go in the audit trail.
        </span>
        <Button variant="secondary" onClick={onClose} disabled={busy}>Cancel</Button>
        <button type="button" className="button" data-busy={busy ? "" : undefined} disabled={!changed || busy || working} onClick={() => void save()}>
          {busy ? <><NxIcon name="sun" size={16} className="nx-spin" />Saving</> : <>Save picture<NxIcon name="arrow-right" size={16} /></>}
        </button>
      </div>
    </Modal>
  );
}
