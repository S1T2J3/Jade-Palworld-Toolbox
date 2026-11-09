import oozBytes from "./ooz-bytes.mjs";

const maxMeta = 2 * 1024 * 1024;
let compiled;

function invalid() {
  throw new Error("无法解读世界元数据");
}

// Also works in a privileged Gecko module without DOM globals.
function fromBase64(text) {
  if (typeof text !== "string" || !text.length || text.length % 4 ||
      text.length > 4 * Math.ceil(maxMeta / 3)) invalid();
  const alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
  const padding = text.endsWith("==") ? 2 : text.endsWith("=") ? 1 : 0;
  const out = new Uint8Array(text.length / 4 * 3 - padding);
  let accumulator = 0, bits = 0, index = 0;
  for (let i = 0; i < text.length - padding; i++) {
    const value = alphabet.indexOf(text[i]);
    if (value === -1) invalid();
    accumulator = (accumulator << 6) | value;
    bits += 6;
    if (bits >= 8) {
      bits -= 8;
      out[index++] = (accumulator >> bits) & 255;
    }
  }
  if (index !== out.length || (accumulator & ((1 << bits) - 1))) invalid();
  return out;
}

function readName(bytes) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let cursor = 0;
  function take(n, end) {
    if (n < 0 || n > end - cursor) invalid();
    const start = cursor;
    cursor += n;
    return start;
  }
  function i32(end) { return view.getInt32(take(4, end), true); }
  function string(end, playerText = false) {
    const count = i32(end);
    if (!count) return "";
    const wide = count < 0;
    const size = wide ? count * -2 : count;
    if (size < 1 || size > 65536) invalid();
    const start = take(size, end);
    const data = bytes.subarray(start, cursor);
    if (data[size - 1] || (wide && data[size - 2])) invalid();
    if (!wide) {
      // All schema identifiers are ASCII. A positive FString may contain UTF-8.
      let escaped = "";
      for (let i = 0; i < size - 1; i++) escaped += "%" + data[i].toString(16).padStart(2, "0");
      return decodeURIComponent(escaped);
    }
    let value = "";
    for (let i = 0; i < size - 2; i += 2) {
      const unit = data[i] | data[i + 1] << 8;
      if (unit >= 0xd800 && unit <= 0xdbff && i + 4 < size) {
        const low = data[i + 2] | data[i + 3] << 8;
        if (low >= 0xdc00 && low <= 0xdfff) {
          value += String.fromCharCode(unit, low);
          i += 2;
          continue;
        }
      }
      if (unit >= 0xd800 && unit <= 0xdfff) {
        if (!playerText) invalid();
        value += "\uFFFD";
      } else value += String.fromCharCode(unit);
    }
    return value;
  }
  function tags(end, inside) {
    while (cursor < end) {
      const name = string(end);
      if (name === "None") break;
      const type = string(end);
      const size = i32(end);
      if (i32(end) !== 0 || size < 0) invalid();
      let structName;
      if (type === "StructProperty") {
        structName = string(end);
        take(16, end);
      } else if (type === "BoolProperty") {
        take(1, end);
      } else if (type === "MapProperty") {
        string(end);
        string(end);
      } else if (["ArrayProperty", "SetProperty", "ByteProperty", "EnumProperty"].includes(type)) string(end);
      const guidFlag = bytes[take(1, end)];
      if (guidFlag === 1) take(16, end);
      else if (guidFlag !== 0) invalid();
      if (size > end - cursor) invalid();
      const next = cursor + size;
      if (!inside && name === "SaveData" && structName === "PalWorldBaseInfoSaveData") return tags(next, true);
      if (inside && name === "WorldName" && type === "StrProperty") {
        const value = string(next, true);
        if (cursor !== next || !value.trim() || value.length > 256 || /[\u0000-\u001F\u007F-\u009F]/u.test(value)) invalid();
        return value;
      }
      cursor = next;
    }
    invalid();
  }
  const end = bytes.byteLength;
  if (i32(end) !== 0x53415647 || i32(end) !== 3) invalid();
  take(18, end);
  string(end);
  if (i32(end) !== 3) invalid();
  const versions = i32(end);
  if (versions < 0 || versions > 4096) invalid();
  take(versions * 20, end);
  if (string(end) !== "/Script/Pal.PalWorldBaseInfoSaveGame") invalid();
  return tags(end, false);
}

export async function decodeWorldName(compressedBase64, rawLength) {
  if (!Number.isInteger(rawLength) || rawLength <= 0 || rawLength > maxMeta) invalid();
  const input = fromBase64(compressedBase64);
  if (!input.length || input.length > maxMeta) invalid();
  if (!compiled) compiled = WebAssembly.compile(fromBase64(oozBytes));
  // Each metadata file has its own linear memory. The codec has no access
  // to the filesystem or network, and its initial 16 MiB heap cannot grow.
  let api;
  const imports = { a: {
    a() { return 0; },
    b(dst, src, count) { new Uint8Array(api.c.buffer).copyWithin(dst >>> 0, src >>> 0, (src >>> 0) + (count >>> 0)); }
  }};
  const instance = await WebAssembly.instantiate(await compiled, imports);
  api = instance.exports;
  api.d(); // Runtime constructors.
  const inPtr = api.e(input.length + 64);
  const outPtr = api.e(rawLength + 64);
  if (!inPtr || !outPtr) invalid();
  const memory = new Uint8Array(api.c.buffer);
  memory.set(input, inPtr);
  if (api.g(inPtr, input.length, outPtr, rawLength) !== rawLength) invalid();
  return readName(memory.subarray(outPtr, outPtr + rawLength));
}
