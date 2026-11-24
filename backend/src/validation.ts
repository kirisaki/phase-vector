export class ApiError extends Error {
  constructor(public status: number, public code: string) { super(code); }
}
export const MAX_IMAGE_BYTES = 2 * 1024 * 1024;
export const MAX_REQUEST_BYTES = MAX_IMAGE_BYTES + 24 * 1024;
export const categories = ['bug', 'controls', 'balance', 'performance', 'other'] as const;
const outcomes = ['running', 'breakthrough', 'disabled', 'own_disabled', 'timeout', 'sandbox'];
export type Kind = 'feedback' | 'result';
type RecordValue = Record<string, unknown>;
function object(value: unknown, keys: string[]): RecordValue {
  if (!value || typeof value !== 'object' || Array.isArray(value)
    || Object.keys(value).some(key => !keys.includes(key))) throw new ApiError(400, 'invalid_fields');
  return value as RecordValue;
}
function text(value: unknown, min: number, max: number): string {
  if (typeof value !== 'string' || value.trim().length < min || value.length > max) throw new ApiError(400, 'invalid_text');
  return value.trim();
}
function number(value: unknown, min: number, max: number): number {
  if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max) throw new ApiError(400, 'invalid_number');
  return value;
}
function choice(value: unknown, allowed: readonly string[]): string {
  if (typeof value !== 'string' || !allowed.includes(value)) throw new ApiError(400, 'invalid_choice');
  return value;
}
function uuid(value: unknown): string {
  if (typeof value !== 'string' || !/^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i.test(value)) throw new ApiError(400, 'invalid_id');
  return value.toLowerCase();
}
export function validatePayload(input: unknown, kind: Kind) {
  const p = object(input, ['sessionId', 'submissionId', 'rating', 'category', 'message', 'metadata']);
  const m = object(p.metadata, ['version', 'outcome', 'runKind', 'simulationSeconds', 'forecastSeconds', 'score', 'shotsFired', 'ownDeltaVSpent', 'viewport']);
  const viewport = object(m.viewport, ['width', 'height']);
  const outcome = choice(m.outcome, outcomes);
  const score = m.score === null ? null : number(m.score, 0, 3000);
  if ((outcome === 'running' || outcome === 'sandbox') !== (score === null)) throw new ApiError(400, 'invalid_score');
  if (kind === 'result' && (score === null || p.rating !== undefined || p.category !== undefined || p.message !== undefined)) throw new ApiError(400, 'invalid_result');
  // Older cached clients may still send a rating. New reports do not collect one.
  const rating = kind === 'feedback' && p.rating !== undefined ? number(p.rating, 1, 5) : undefined;
  if (rating !== undefined && !Number.isInteger(rating)) throw new ApiError(400, 'invalid_rating');
  return {
    sessionId: uuid(p.sessionId), submissionId: uuid(p.submissionId),
    ...(kind === 'feedback' ? { ...(rating === undefined ? {} : { rating }), category: choice(p.category, categories), message: text(p.message, 1, 4000) } : {}),
    metadata: {
      version: text(m.version, 1, 80), outcome,
      runKind: choice(m.runKind, ['standard', 'custom', 'debug', 'sandbox']),
      simulationSeconds: number(m.simulationSeconds, 0, 1e9),
      forecastSeconds: m.forecastSeconds === null ? null : number(m.forecastSeconds, 0, 1e9),
      score, shotsFired: number(m.shotsFired, 0, 1e6), ownDeltaVSpent: number(m.ownDeltaVSpent, 0, 1e9),
      viewport: { width: number(viewport.width, 1, 16384), height: number(viewport.height, 1, 16384) },
    },
  };
}
// Bound reads even when Content-Length is absent or untrusted.
export async function boundedBody(request: Request): Promise<ArrayBuffer> {
  if (Number(request.headers.get('content-length')) > MAX_REQUEST_BYTES) throw new ApiError(413, 'request_too_large');
  const reader = request.body?.getReader();
  if (!reader) throw new ApiError(400, 'empty_request');
  const chunks: Uint8Array[] = [];
  let size = 0;
  while (true) {
    const { done, value } = await reader.read();
    if (done) break;
    size += value.length;
    if (size > MAX_REQUEST_BYTES) { await reader.cancel(); throw new ApiError(413, 'request_too_large'); }
    chunks.push(value);
  }
  const data = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) { data.set(chunk, offset); offset += chunk.length; }
  return data.buffer;
}
const crcTable = Uint32Array.from({ length: 256 }, (_, byte) => {
  let crc = byte;
  for (let bit = 0; bit < 8; bit++) crc = (crc >>> 1) ^ (0xedb88320 & -(crc & 1));
  return crc >>> 0;
});
function crc32(bytes: Uint8Array): number {
  let crc = 0xffffffff;
  for (const byte of bytes) crc = (crc >>> 8) ^ crcTable[(crc ^ byte) & 255];
  return (crc ^ 0xffffffff) >>> 0;
}
export async function validateImage(file: File): Promise<ArrayBuffer> {
  if (file.type !== 'image/png') throw new ApiError(415, 'png_required');
  if (!file.size || file.size > MAX_IMAGE_BYTES) throw new ApiError(413, 'image_too_large');
  const buffer = await file.arrayBuffer(), bytes = new Uint8Array(buffer), view = new DataView(buffer);
  if (bytes.length < 57 || [137,80,78,71,13,10,26,10].some((b,i) => bytes[i] !== b)) throw new ApiError(415, 'invalid_png');
  let offset = 8, width = 0, height = 0, channels = 0, ended = false;
  const compressed: ArrayBuffer[] = [];
  while (offset + 12 <= bytes.length) {
    const length = view.getUint32(offset), end = offset + 12 + length;
    if (end > bytes.length || length > MAX_IMAGE_BYTES) throw new ApiError(415, 'invalid_png');
    const type = String.fromCharCode(...bytes.subarray(offset + 4, offset + 8));
    if (crc32(bytes.subarray(offset + 4, end - 4)) !== view.getUint32(end - 4)) throw new ApiError(415, 'invalid_png');
    if (offset === 8) {
      if (type !== 'IHDR' || length !== 13) throw new ApiError(415, 'invalid_png');
      width = view.getUint32(offset + 8); height = view.getUint32(offset + 12);
      const color = bytes[offset + 17]; channels = color === 6 ? 4 : color === 2 ? 3 : 0;
      if (!width || !height || width > 4096 || height > 4096 || width * height > 4_000_000 || !channels
        || bytes[offset + 16] !== 8 || bytes[offset + 18] || bytes[offset + 19] || bytes[offset + 20]) throw new ApiError(415, 'unsupported_png');
    } else if (type === 'IDAT') compressed.push(buffer.slice(offset + 8, end - 4));
    else if (type === 'IEND') { if (length || end !== bytes.length) throw new ApiError(415, 'invalid_png'); ended = true; }
    // Do not retain EXIF, text, or arbitrary ancillary payloads in screenshots.
    else if (!['sRGB', 'gAMA', 'cHRM', 'pHYs'].includes(type)) throw new ApiError(415, 'unsupported_png');
    offset = end;
  }
  if (!ended || !compressed.length) throw new ApiError(415, 'invalid_png');
  try {
    const reader = new Blob(compressed).stream().pipeThrough(new DecompressionStream('deflate')).getReader();
    const rowBytes = width * channels + 1, expected = rowBytes * height;
    let size = 0;
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      for (let i = (rowBytes - size % rowBytes) % rowBytes; i < value.length; i += rowBytes) {
        if (value[i] > 4) { await reader.cancel(); throw new Error('filter'); }
      }
      size += value.length;
      if (size > expected) { await reader.cancel(); throw new Error('pixels'); }
    }
    if (size !== expected) throw new Error('pixels');
  } catch { throw new ApiError(415, 'invalid_png'); }
  return buffer;
}
