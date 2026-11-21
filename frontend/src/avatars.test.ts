import assert from "node:assert/strict";
import test from "node:test";
import { MOTIFS, MOTIF_NAMES, TONES, automaticPicture, pictureFor, squareCrop } from "./avatars.ts";

test("a person's automatic picture never changes and is one of the Nexus defaults", () => {
  const id = "0b6f6a34-5a0e-4d55-9f0c-1b2d3c4e5f60";
  const first = automaticPicture(id);
  assert.deepEqual(automaticPicture(id), first);
  assert.ok((MOTIFS as readonly string[]).includes(first.motif));
  assert.ok(first.tone >= 0 && first.tone < TONES);
});

test("a roster of people is not one repeated mark", () => {
  const ids = Array.from({ length: 200 }, (_, index) => `00000000-0000-4000-8000-${String(index).padStart(12, "0")}`);
  const pictures = ids.map(automaticPicture);
  const motifs = new Set(pictures.map((picture) => picture.motif));
  const pairs = new Set(pictures.map((picture) => `${picture.motif}/${picture.tone}`));
  assert.equal(motifs.size, MOTIFS.length, "every motif is used");
  assert.ok(pairs.size >= 40, `only ${pairs.size} distinct pictures in 200 people`);
  // No single motif takes more than a quarter of the roster.
  for (const motif of MOTIFS) assert.ok(pictures.filter((picture) => picture.motif === motif).length <= 50, motif);
  // Neighbouring ids, as in a seeded list, do not all land on the same picture.
  const neighbours = new Set(pictures.slice(0, 8).map((picture) => `${picture.motif}/${picture.tone}`));
  assert.ok(neighbours.size >= 5, `${neighbours.size} distinct pictures in the first eight`);
});

test("a chosen default keeps the person's colour; a photo keeps a default to fall back on", () => {
  const id = "client-1";
  const automatic = automaticPicture(id);
  assert.deepEqual(pictureFor(id, undefined), { kind: "motif", ...automatic, chosen: false });
  assert.deepEqual(pictureFor(id, ""), { kind: "motif", ...automatic, chosen: false });
  assert.deepEqual(pictureFor(id, "fern"), { kind: "motif", motif: "fern", tone: automatic.tone, chosen: true });
  assert.deepEqual(pictureFor(id, "photo:0123456789abcdef"), { kind: "photo", tag: "0123456789abcdef", ...automatic });
});

test("anything that is not a known choice falls back to the automatic picture", () => {
  const automatic = { kind: "motif", ...automaticPicture("x"), chosen: false };
  for (const choice of ["dragon", "photo:", "photo:XYZ", "photo:0123", "FERN", "javascript:alert(1)", "https://example.invalid/a.png"]) {
    assert.deepEqual(pictureFor("x", choice), automatic, choice);
  }
});

test("every motif has a name where it is chosen", () => {
  assert.deepEqual(Object.keys(MOTIF_NAMES).sort(), [...MOTIFS].sort());
});

test("a photo is cropped to the largest centred square", () => {
  assert.deepEqual(squareCrop(1200, 800), { x: 200, y: 0, side: 800 });
  assert.deepEqual(squareCrop(600, 901), { x: 0, y: 150, side: 600 });
  assert.deepEqual(squareCrop(256, 256), { x: 0, y: 0, side: 256 });
  assert.deepEqual(squareCrop(0, 10), { x: 0, y: 5, side: 0 });
});
