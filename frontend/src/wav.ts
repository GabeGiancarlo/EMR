/** Pure RIFF/WAVE checks matching api/src/speech.rs before a recording is uploaded. */

export const MAX_WAV_BYTES = 20 * 1024 * 1024;
export const MAX_DURATION_SECS = 600;
const SAMPLE_RATE = 16_000;

export const WAV_ERROR_MESSAGE = {
  too_large: "The recording exceeds 20 MiB.",
  not_riff_wave: "Send a RIFF/WAVE file.",
  chunk_bounds: "The WAV chunk sizes do not fit inside the file.",
  not_pcm16_mono_16k: "The recording must be PCM 16-bit, mono, 16000 Hz.",
  duration: "The recording must be 10 minutes or shorter.",
  empty: "The recording has no audio samples.",
} as const;

export type WavErrorCode = keyof typeof WAV_ERROR_MESSAGE;

export type WavValidation =
  | { ok: true; dataBytes: number }
  | { ok: false; code: WavErrorCode; message: string };

function fail(code: WavErrorCode): WavValidation {
  return { ok: false, code, message: WAV_ERROR_MESSAGE[code] };
}

function matchesAscii(bytes: Uint8Array, offset: number, text: string): boolean {
  for (let i = 0; i < text.length; i += 1) {
    if (bytes[offset + i] !== text.charCodeAt(i)) return false;
  }
  return true;
}

export function validateWav(
  input: ArrayBuffer | Uint8Array,
  maxBytes = MAX_WAV_BYTES,
  maxSecs = MAX_DURATION_SECS,
): WavValidation {
  const bytes = input instanceof Uint8Array ? input : new Uint8Array(input);
  if (bytes.length > maxBytes) return fail("too_large");
  if (
    bytes.length < 12 ||
    !matchesAscii(bytes, 0, "RIFF") ||
    !matchesAscii(bytes, 8, "WAVE")
  ) {
    return fail("not_riff_wave");
  }

  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const end = view.getUint32(4, true) + 8;
  if (end !== bytes.length) return fail("chunk_bounds");

  let offset = 12;
  let fmt: {
    formatTag: number;
    channels: number;
    rate: number;
    byteRate: number;
    blockAlign: number;
    bits: number;
  } | null = null;
  let dataBytes: number | null = null;

  while (offset < end) {
    if (offset + 8 > end) return fail("chunk_bounds");
    const size = view.getUint32(offset + 4, true);
    const dataStart = offset + 8;
    const dataEnd = dataStart + size;
    if (!Number.isSafeInteger(dataEnd) || dataEnd > end) return fail("chunk_bounds");
    const padded = size % 2 === 1 ? dataEnd + 1 : dataEnd;
    if (padded > end) return fail("chunk_bounds");

    if (matchesAscii(bytes, offset, "fmt ")) {
      if (fmt !== null) return fail("chunk_bounds");
      if (size < 16) return fail("not_pcm16_mono_16k");
      fmt = {
        formatTag: view.getUint16(dataStart, true),
        channels: view.getUint16(dataStart + 2, true),
        rate: view.getUint32(dataStart + 4, true),
        byteRate: view.getUint32(dataStart + 8, true),
        blockAlign: view.getUint16(dataStart + 12, true),
        bits: view.getUint16(dataStart + 14, true),
      };
    } else if (matchesAscii(bytes, offset, "data")) {
      if (dataBytes !== null) return fail("chunk_bounds");
      dataBytes = size;
    }

    offset = padded;
  }

  if (offset !== end) return fail("chunk_bounds");
  if (fmt === null || dataBytes === null) return fail("not_pcm16_mono_16k");
  if (
    fmt.formatTag !== 1 ||
    fmt.channels !== 1 ||
    fmt.rate !== SAMPLE_RATE ||
    fmt.bits !== 16 ||
    fmt.blockAlign !== 2 ||
    fmt.byteRate !== SAMPLE_RATE * 2
  ) {
    return fail("not_pcm16_mono_16k");
  }
  if (dataBytes === 0) return fail("empty");
  if (dataBytes % 2 !== 0) return fail("not_pcm16_mono_16k");
  if (Math.floor(dataBytes / 2) > fmt.rate * maxSecs) return fail("duration");
  return { ok: true, dataBytes };
}
