import { mkdir, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import opentype from "opentype.js";

const desktopRoot = resolve(import.meta.dirname, "..");
const brandRoot = resolve(desktopRoot, "assets/brand/watchback");
const iconRoot = resolve(desktopRoot, "src-tauri/icons");
const fontPath = "C:\\Windows\\Fonts\\segoeui.ttf";

const COLORS = {
  ink: "#202220",
  purple: "#4B22D1",
  warmWhite: "#F4F6F0",
  canvas: "#E3E8E2",
  secondary: "#60645F",
};

const STANDARD_PATHS = [
  "M77 25C64 13 43 14 29 27C15 40 14 61 27 75C38 87 57 89 71 79",
  "M69 39C60 31 47 32 39 40C31 48 31 61 39 69C47 77 59 77 67 70",
  "M61 53C57 49 51 49 47 53C43 57 44 63 48 66",
];

const SMALL_PATHS = [
  "M77 25C64 13 43 14 29 27C15 40 14 61 27 75C38 87 57 89 71 79",
  "M69 39C60 31 47 32 39 40C31 48 31 61 39 69C47 77 59 77 67 70",
  "M60 53C56 50 51 50 48 54C45 58 46 63 50 66",
];

function mark({ ink, node, strokeWidth = 10, paths = STANDARD_PATHS }) {
  return [
    ...paths.map(
      (d) =>
        `<path d="${d}" fill="none" stroke="${ink}" stroke-width="${strokeWidth}" stroke-linecap="round"/>`,
    ),
    `<circle cx="77" cy="25" r="${strokeWidth / 2}" fill="${node}"/>`,
  ].join("");
}

function svg({
  body,
  viewBox = "0 0 96 96",
  width = 96,
  height = 96,
  label = "Watchback",
}) {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="${height}" viewBox="${viewBox}" role="img" aria-label="${label}">${body}</svg>\n`;
}

function pathData(font, text, x, y, size) {
  return font.getPath(text, x, y, size).toPathData(2);
}

function placed(contents, x, y, size) {
  const scale = size / 96;
  return `<g transform="translate(${x} ${y}) scale(${scale})">${contents}</g>`;
}

function label(text, x, y, fill = COLORS.secondary) {
  return `<text x="${x}" y="${y}" fill="${fill}" font-family="Segoe UI, sans-serif" font-size="12" font-weight="400">${text}</text>`;
}

async function main() {
  await mkdir(brandRoot, { recursive: true });
  await mkdir(iconRoot, { recursive: true });
  const fontBuffer = await readFile(fontPath);
  const font = opentype.parse(fontBuffer.buffer);

  const standard = mark({ ink: COLORS.ink, node: COLORS.purple });
  const mono = mark({ ink: COLORS.ink, node: COLORS.ink });
  const inverse = mark({ ink: COLORS.warmWhite, node: COLORS.warmWhite });
  const small16 = mark({
    ink: COLORS.ink,
    node: COLORS.purple,
    strokeWidth: 11.5,
    paths: SMALL_PATHS,
  });
  const small24 = mark({
    ink: COLORS.ink,
    node: COLORS.purple,
    strokeWidth: 11,
    paths: SMALL_PATHS,
  });

  const wordmark = pathData(font, "Watchback", 118, 64, 34);
  const logo = `<g transform="translate(8 8)">${standard}</g><path id="watchback-wordmark" d="${wordmark}" fill="${COLORS.ink}"/>`;
  const descriptor = `<text x="118" y="88" fill="${COLORS.secondary}" font-family="Microsoft YaHei UI, PingFang SC, Noto Sans CJK SC, Segoe UI, sans-serif" font-size="13" font-weight="400" letter-spacing="1">守望战绩工具</text>`;

  const smallSizes = [16, 24, 32, 48, 64, 128];
  let smallX = 32;
  const smallPreview = smallSizes
    .map((size) => {
      const source = size <= 24 ? (size === 16 ? small16 : small24) : standard;
      const item = `${placed(source, smallX, 214 - size, size)}${label(`${size}px`, smallX, 232)}`;
      smallX += Math.max(size, 38) + 18;
      return item;
    })
    .join("");

  const reviewSheet = [
    `<rect width="640" height="400" fill="#FFFFFF"/>`,
    `<rect x="20" y="20" width="180" height="170" rx="20" fill="${COLORS.warmWhite}"/>`,
    placed(standard, 46, 40, 128),
    label("standard C2", 64, 176),
    `<rect x="230" y="20" width="180" height="170" rx="20" fill="#FFFFFF" stroke="#D7DAD5"/>`,
    placed(mono, 256, 40, 128),
    label("monochrome", 276, 176),
    `<rect x="440" y="20" width="180" height="170" rx="20" fill="${COLORS.ink}"/>`,
    placed(inverse, 466, 40, 128),
    label("inverse", 504, 176, COLORS.warmWhite),
    smallPreview,
    `<g transform="translate(20 258) scale(0.72)">${logo}</g>`,
    `<g transform="translate(20 324) scale(0.72)">${logo}${descriptor}</g>`,
  ].join("");

  const files = new Map([
    [resolve(brandRoot, "watchback-mark.svg"), svg({ body: standard })],
    [resolve(brandRoot, "watchback-mark-monochrome.svg"), svg({ body: mono })],
    [resolve(brandRoot, "watchback-mark-inverse.svg"), svg({ body: inverse })],
    [resolve(brandRoot, "watchback-mark-16.svg"), svg({ body: small16 })],
    [resolve(brandRoot, "watchback-mark-24.svg"), svg({ body: small24 })],
    [
      resolve(brandRoot, "watchback-logo.svg"),
      svg({ body: logo, viewBox: "0 0 320 112", width: 320, height: 112 }),
    ],
    [
      resolve(brandRoot, "watchback-logo-with-descriptor.svg"),
      svg({
        body: `${logo}${descriptor}`,
        viewBox: "0 0 320 112",
        width: 320,
        height: 112,
        label: "Watchback 守望战绩工具",
      }),
    ],
    [
      resolve(brandRoot, "watchback-review-sheet.svg"),
      svg({
        body: reviewSheet,
        viewBox: "0 0 640 400",
        width: 640,
        height: 400,
        label: "Watchback brand review sheet",
      }),
    ],
    [
      resolve(iconRoot, "watchback-icon-source.svg"),
      svg({
        body: `<rect width="96" height="96" rx="22" fill="${COLORS.warmWhite}"/>${standard}`,
        width: 1024,
        height: 1024,
      }),
    ],
  ]);

  for (const [file, contents] of files) {
    await writeFile(file, contents, "utf8");
  }
}

await main();
