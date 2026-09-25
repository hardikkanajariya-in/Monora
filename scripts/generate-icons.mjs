import { readFileSync, writeFileSync, mkdirSync, copyFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = join(__dirname, "..");
const svgPath = join(root, "assets", "brand", "monora-icon.svg");
const svg = readFileSync(svgPath, "utf8");

const iconsDir = join(root, "src-tauri", "icons");
mkdirSync(iconsDir, { recursive: true });

const sizes = [
  { name: "32x32.png", size: 32 },
  { name: "128x128.png", size: 128 },
  { name: "128x128@2x.png", size: 256 },
  { name: "icon.png", size: 512 },
  { name: "Square30x30Logo.png", size: 30 },
  { name: "Square44x44Logo.png", size: 44 },
  { name: "Square71x71Logo.png", size: 71 },
  { name: "Square89x89Logo.png", size: 89 },
  { name: "Square107x107Logo.png", size: 107 },
  { name: "Square142x142Logo.png", size: 142 },
  { name: "Square150x150Logo.png", size: 150 },
  { name: "Square284x284Logo.png", size: 284 },
  { name: "Square310x310Logo.png", size: 310 },
  { name: "StoreLogo.png", size: 50 },
];

function renderPng(size) {
  const resvg = new Resvg(svg, {
    fitTo: { mode: "width", value: size },
    background: "transparent",
  });
  return resvg.render().asPng();
}

for (const { name, size } of sizes) {
  writeFileSync(join(iconsDir, name), renderPng(size));
  console.log(`Wrote ${name} (${size}px)`);
}

const png1024 = renderPng(1024);
writeFileSync(join(root, "assets", "brand", "monora-icon-1024.png"), png1024);

const publicDir = join(root, "public");
const websiteAssets = join(root, "website", "assets");
mkdirSync(publicDir, { recursive: true });
mkdirSync(websiteAssets, { recursive: true });

copyFileSync(svgPath, join(publicDir, "monora-icon.svg"));
copyFileSync(svgPath, join(websiteAssets, "monora-icon.svg"));
copyFileSync(join(root, "assets", "brand", "monora-folder.svg"), join(websiteAssets, "monora-folder.svg"));
copyFileSync(join(root, "assets", "brand", "monora-folder.svg"), join(publicDir, "monora-folder.svg"));

console.log("Synced SVG assets to public/ and website/assets/");
