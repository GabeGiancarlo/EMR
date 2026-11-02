import { strict as assert } from "node:assert";
import { test } from "node:test";
import {
  MAX_DURATION_SECS,
  MAX_WAV_BYTES,
  WAV_ERROR_MESSAGE,
  validateWav,
} from "./wav.ts";

function pushAscii(buf: number[], text: string) {
  for (let i = 0; i < text.length; i += 1) buf.push(text.charCodeAt(i));
}

function pushU16(buf: number[], n: number) {
  buf.push(n & 0xff, (n >>> 8) & 0xff);
}

function pushU32(buf: number[], n: number) {
  buf.push(n & 0xff, (n >>> 8) & 0xff, (n >>> 16) & 0xff, (n >>> 24) & 0xff);
}

function pushChunk(buf: number[], id: string, data: number[], pad = true) {
  pushAscii(buf, id);
  pushU32(buf, data.length);
  buf.push(...data);
  if (pad && data.length % 2 === 1) buf.push(0);
}

function wrapWave(chunks: number[]): Uint8Array {
  const file: number[] = [];
  pushAscii(file, "RIFF");
  pushU32(file, chunks.length + 4);
  pushAscii(file, "WAVE");
  file.push(...chunks);
  return Uint8Array.from(file);
}

function fmtChunk(
  format = 1,
  channels = 1,
  rate = 16_000,
  bits = 16,
  byteRate = rate * channels * Math.floor(bits / 8),
  block = channels * Math.floor(bits / 8),
  extra: number[] = [],
): number[] {
  const data: number[] = [];
  pushU16(data, format);
  pushU16(data, channels);
  pushU32(data, rate);
  pushU32(data, byteRate);
  pushU16(data, block);
  pushU16(data, bits);
  data.push(...extra);
  const chunk: number[] = [];
  pushChunk(chunk, "fmt ", data);
  return chunk;
}

function wavWith(
  format: number,
  channels: number,
  rate: number,
  bits: number,
  samples: number,
): Uint8Array {
  const block = channels * Math.floor(bits / 8);
  const chunks = fmtChunk(format, channels, rate, bits);
  pushChunk(chunks, "data", Array.from({ length: samples * block }, () => 0));
  return wrapWave(chunks);
}

function validWav(samples: number): Uint8Array {
  return wavWith(1, 1, 16_000, 16, samples);
}

function expectError(
  bytes: Uint8Array,
  code: keyof typeof WAV_ERROR_MESSAGE,
  maxBytes?: number,
  maxSecs?: number,
) {
  assert.deepEqual(validateWav(bytes, maxBytes, maxSecs), {
    ok: false,
    code,
    message: WAV_ERROR_MESSAGE[code],
  });
}

test("accepts pcm16 mono 16k including extra, extended, and reordered chunks", () => {
  assert.deepEqual(validateWav(validWav(1)), { ok: true, dataBytes: 2 });

  const chunks = [...fmtChunk()];
  pushChunk(chunks, "fact", [4, 0, 0, 0]);
  pushChunk(chunks, "data", [0, 0]);
  pushChunk(chunks, "LIST", [97, 98, 99]);
  assert.deepEqual(validateWav(wrapWave(chunks)), { ok: true, dataBytes: 2 });

  const reordered: number[] = [];
  pushChunk(reordered, "data", [9, 8]);
  reordered.push(...fmtChunk());
  assert.deepEqual(validateWav(wrapWave(reordered)), { ok: true, dataBytes: 2 });

  const extended = fmtChunk(1, 1, 16_000, 16, 32_000, 2, [0, 0]);
  pushChunk(extended, "data", [1, 2]);
  assert.deepEqual(validateWav(wrapWave(extended)), { ok: true, dataBytes: 2 });

  const wav = validWav(1);
  const padded = new Uint8Array(wav.length + 5);
  padded.set(wav, 3);
  assert.deepEqual(validateWav(padded.subarray(3, 3 + wav.length)), {
    ok: true,
    dataBytes: 2,
  });
  const exact = wav.buffer.slice(wav.byteOffset, wav.byteOffset + wav.byteLength);
  assert.ok(exact instanceof ArrayBuffer);
  assert.deepEqual(validateWav(exact), { ok: true, dataBytes: 2 });
});

test("rejects non-wav and wrong pcm layout", () => {
  expectError(
    Uint8Array.from("not a wav file!!".split("").map((c) => c.charCodeAt(0))),
    "not_riff_wave",
  );
  const partial = new Uint8Array(11);
  partial.set([82, 73, 70, 70]);
  expectError(partial, "not_riff_wave");
  const lowerWave = validWav(1);
  lowerWave[8] = "w".charCodeAt(0);
  expectError(lowerWave, "not_riff_wave");

  expectError(wavWith(3, 1, 16_000, 16, 1), "not_pcm16_mono_16k");
  expectError(wavWith(1, 2, 16_000, 16, 1), "not_pcm16_mono_16k");
  expectError(wavWith(1, 1, 44_100, 16, 1), "not_pcm16_mono_16k");
  expectError(wavWith(1, 1, 16_000, 8, 1), "not_pcm16_mono_16k");
  expectError(wavWith(1, 1, 16_000, 16, 0), "empty");

  const shortFmt: number[] = [];
  pushChunk(shortFmt, "fmt ", [1, 0, 1, 0]);
  pushChunk(shortFmt, "data", [0, 0]);
  expectError(wrapWave(shortFmt), "not_pcm16_mono_16k");
  expectError(wrapWave(fmtChunk()), "not_pcm16_mono_16k");
  const onlyData: number[] = [];
  pushChunk(onlyData, "data", [0, 0]);
  expectError(wrapWave(onlyData), "not_pcm16_mono_16k");

  const oddData = [...fmtChunk()];
  pushChunk(oddData, "data", [0]);
  expectError(wrapWave(oddData), "not_pcm16_mono_16k");
});

test("inconsistent byte rate or block alignment is rejected", () => {
  const rate = validWav(1);
  assert.equal(
    new DataView(rate.buffer, rate.byteOffset, rate.byteLength).getUint32(28, true),
    32_000,
  );
  rate[28] ^= 0xff;
  expectError(rate, "not_pcm16_mono_16k");

  const block = validWav(1);
  assert.equal(
    new DataView(block.buffer, block.byteOffset, block.byteLength).getUint16(32, true),
    2,
  );
  block[32] ^= 0xff;
  expectError(block, "not_pcm16_mono_16k");
});

test("riff length and chunk bounds reject truncated, unpadded, and duplicate chunks", () => {
  const declared = [...fmtChunk()];
  pushAscii(declared, "data");
  pushU32(declared, 100);
  declared.push(0, 0);
  expectError(wrapWave(declared), "chunk_bounds");

  const odd = [...fmtChunk()];
  pushChunk(odd, "data", [0, 0]);
  pushAscii(odd, "LIST");
  pushU32(odd, 3);
  pushAscii(odd, "abc");
  expectError(wrapWave(odd), "chunk_bounds");

  const huge = wrapWave([0, 0, 0, 0, 0, 0, 0, 0]);
  huge.set([0xff, 0xff, 0xff, 0xff], 16);
  expectError(huge, "chunk_bounds");

  const extra = validWav(1);
  const trailed = new Uint8Array(extra.length + 1);
  trailed.set(extra);
  expectError(trailed, "chunk_bounds");

  const full = validWav(1);
  expectError(full.subarray(0, full.length - 1), "chunk_bounds");
  const grown = validWav(1);
  const grownView = new DataView(grown.buffer, grown.byteOffset, grown.byteLength);
  grownView.setUint32(4, grownView.getUint32(4, true) + 50, true);
  expectError(grown, "chunk_bounds");

  const dupFmt = [...fmtChunk(), ...fmtChunk()];
  pushChunk(dupFmt, "data", [0, 0]);
  expectError(wrapWave(dupFmt), "chunk_bounds");
  const dupData = [...fmtChunk()];
  pushChunk(dupData, "data", [0, 0]);
  pushChunk(dupData, "data", [0, 0]);
  expectError(wrapWave(dupData), "chunk_bounds");
});

test("duration cap is inclusive and the 20 MiB cap matches the API", () => {
  assert.equal(MAX_WAV_BYTES, 20 * 1024 * 1024);
  assert.equal(MAX_DURATION_SECS, 600);
  assert.ok(16_000 * 2 * MAX_DURATION_SECS + 128 <= MAX_WAV_BYTES);
  assert.match(WAV_ERROR_MESSAGE.duration, /10 minutes/);
  assert.equal(WAV_ERROR_MESSAGE.chunk_bounds.includes("RIFF"), false);
  assert.match(WAV_ERROR_MESSAGE.too_large, /20 MiB/);
  assert.match(WAV_ERROR_MESSAGE.empty, /no audio samples/);
  assert.match(WAV_ERROR_MESSAGE.not_pcm16_mono_16k, /16000 Hz/);

  assert.deepEqual(validateWav(validWav(16_000), Number.MAX_SAFE_INTEGER, 1), {
    ok: true,
    dataBytes: 32_000,
  });
  expectError(validWav(16_001), "duration", Number.MAX_SAFE_INTEGER, 1);
  expectError(validWav(1), "too_large", 10, 600);
  assert.deepEqual(validateWav(validWav(16_000 * 2)), { ok: true, dataBytes: 64_000 });
  expectError(validWav(16_000 * 2), "duration", MAX_WAV_BYTES, 1);

  expectError(new Uint8Array(13), "too_large", 12);
  expectError(new Uint8Array(12), "not_riff_wave", 12);
});
