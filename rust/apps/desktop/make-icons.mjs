// Generates the Tauri application icon set without external dependencies.
//
//   node make-icons.mjs
//
// Writes PNGs (plus a multi-size .ico) into `src-tauri/icons/`, matching the
// paths listed in `src-tauri/tauri.conf.json`.

import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const outDir = join(here, "src-tauri", "icons");

// Design space (512x512): rounded square + a download glyph.
const DESIGN = 512.0;
const CORNER_RADIUS = 96.0;
const ACCENT = [0, 103, 192];
const WHITE = [255, 255, 255];

const STEM = { x0: 226, x1: 286, y0: 128, y1: 260 };
const HEAD = [
  [156, 244],
  [356, 244],
  [256, 352],
];
const TRAY = { x0: 148, x1: 364, y0: 392, y1: 426 };

function insideRoundedSquare(x, y) {
  if (x < 0 || y < 0 || x > DESIGN || y > DESIGN) return false;
  const r = CORNER_RADIUS;
  const corners = [
    [r, r],
    [DESIGN - r, r],
    [DESIGN - r, DESIGN - r],
    [r, DESIGN - r],
  ];
  for (const [cx, cy] of corners) {
    const inCornerBox =
      (cx < r ? x < r : x > DESIGN - r) && (cy < r ? y < r : y > DESIGN - r);
    if (inCornerBox) {
      const dx = x - cx;
      const dy = y - cy;
      return dx * dx + dy * dy <= r * r;
    }
  }
  return true;
}

function insideTriangle(x, y) {
  const [[x0, y0], [x1, y1], [x2, y2]] = HEAD;
  const d1 = (x - x1) * (y0 - y1) - (x0 - x1) * (y - y1);
  const d2 = (x - x2) * (y1 - y2) - (x1 - x2) * (y - y2);
  const d3 = (x - x0) * (y2 - y0) - (x2 - x0) * (y - y0);
  const hasNeg = d1 < 0 || d2 < 0 || d3 < 0;
  const hasPos = d1 > 0 || d2 > 0 || d3 > 0;
  return !(hasNeg && hasPos);
}

function onGlyph(x, y) {
  if (x >= STEM.x0 && x <= STEM.x1 && y >= STEM.y0 && y <= STEM.y1) return true;
  if (x >= TRAY.x0 && x <= TRAY.x1 && y >= TRAY.y0 && y <= TRAY.y1) return true;
  return insideTriangle(x, y);
}

/** Renders the icon at `size` px with 4x supersampling; returns RGBA bytes. */
function render(size) {
  const ss = 4;
  const scale = DESIGN / size;
  const rgba = Buffer.alloc(size * size * 4);

  for (let py = 0; py < size; py += 1) {
    for (let px = 0; px < size; px += 1) {
      let hits = 0;
      let r = 0;
      let g = 0;
      let b = 0;

      for (let sy = 0; sy < ss; sy += 1) {
        for (let sx = 0; sx < ss; sx += 1) {
          const x = (px + (sx + 0.5) / ss) * scale;
          const y = (py + (sy + 0.5) / ss) * scale;
          if (!insideRoundedSquare(x, y)) continue;
          hits += 1;
          const color = onGlyph(x, y) ? WHITE : ACCENT;
          r += color[0];
          g += color[1];
          b += color[2];
        }
      }

      const index = (py * size + px) * 4;
      if (hits === 0) {
        rgba[index] = 0;
        rgba[index + 1] = 0;
        rgba[index + 2] = 0;
        rgba[index + 3] = 0;
      } else {
        rgba[index] = Math.round(r / hits);
        rgba[index + 1] = Math.round(g / hits);
        rgba[index + 2] = Math.round(b / hits);
        rgba[index + 3] = Math.round((hits / (ss * ss)) * 255);
      }
    }
  }

  return rgba;
}

const CRC_TABLE = (() => {
  const table = new Uint32Array(256);
  for (let n = 0; n < 256; n += 1) {
    let c = n;
    for (let k = 0; k < 8; k += 1) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    table[n] = c >>> 0;
  }
  return table;
})();

function crc32(buffer) {
  let c = 0xffffffff;
  for (const byte of buffer) c = CRC_TABLE[(c ^ byte) & 0xff] ^ (c >>> 8);
  return (c ^ 0xffffffff) >>> 0;
}

function chunk(type, data) {
  const length = Buffer.alloc(4);
  length.writeUInt32BE(data.length, 0);
  const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
  const crc = Buffer.alloc(4);
  crc.writeUInt32BE(crc32(body), 0);
  return Buffer.concat([length, body, crc]);
}

function encodePng(size, rgba) {
  const stride = size * 4;
  const raw = Buffer.alloc((stride + 1) * size);
  for (let y = 0; y < size; y += 1) {
    raw[y * (stride + 1)] = 0;
    rgba.copy(raw, y * (stride + 1) + 1, y * stride, (y + 1) * stride);
  }

  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(size, 0);
  ihdr.writeUInt32BE(size, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 6; // colour type: RGBA
  ihdr[10] = 0;
  ihdr[11] = 0;
  ihdr[12] = 0;

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", ihdr),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

function encodeIco(entries) {
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0);
  header.writeUInt16LE(1, 2);
  header.writeUInt16LE(entries.length, 4);

  const directory = Buffer.alloc(16 * entries.length);
  let offset = 6 + 16 * entries.length;
  entries.forEach((entry, index) => {
    const base = index * 16;
    const dimension = entry.size >= 256 ? 0 : entry.size;
    directory[base] = dimension;
    directory[base + 1] = dimension;
    directory[base + 2] = 0;
    directory[base + 3] = 0;
    directory.writeUInt16LE(1, base + 4);
    directory.writeUInt16LE(32, base + 6);
    directory.writeUInt32LE(entry.png.length, base + 8);
    directory.writeUInt32LE(offset, base + 12);
    offset += entry.png.length;
  });

  return Buffer.concat([header, directory, ...entries.map((entry) => entry.png)]);
}

mkdirSync(outDir, { recursive: true });

const png32 = encodePng(32, render(32));
const png128 = encodePng(128, render(128));
const png256 = encodePng(256, render(256));
const png512 = encodePng(512, render(512));

writeFileSync(join(outDir, "32x32.png"), png32);
writeFileSync(join(outDir, "128x128.png"), png128);
writeFileSync(join(outDir, "128x128@2x.png"), png256);
writeFileSync(join(outDir, "icon.png"), png512);

const icoEntries = [16, 32, 48, 64, 128, 256].map((size) => ({
  size,
  png: encodePng(size, render(size)),
}));
writeFileSync(join(outDir, "icon.ico"), encodeIco(icoEntries));

console.log(`wrote icons to ${outDir}`);