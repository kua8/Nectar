import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+$/.test(version ?? "")) {
  console.error("Usage: bun run bump <major.minor.patch>");
  process.exit(1);
}

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function updateJson(relativePath) {
  const path = resolve(root, relativePath);
  const json = JSON.parse(readFileSync(path, "utf8"));
  json.version = version;
  writeFileSync(path, JSON.stringify(json, null, 2) + "\n");
}

function updateCargoToml() {
  const path = resolve(root, "src-tauri/Cargo.toml");
  const content = readFileSync(path, "utf8");
  writeFileSync(path, content.replace(/^version = "[^"]*"/m, `version = "${version}"`));
}

function updateCargoLock(relativePath, packageName) {
  const path = resolve(root, relativePath);
  const content = readFileSync(path, "utf8");
  const pattern = new RegExp(`(\\[\\[package\\]\\]\\nname = "${packageName}"\\nversion = ")[^"]*(")`);
  writeFileSync(path, content.replace(pattern, `$1${version}$2`));
}

function updateInstallerCargoToml() {
  const path = resolve(root, "installer-app/src-tauri/Cargo.toml");
  const content = readFileSync(path, "utf8");
  writeFileSync(path, content.replace(/^version = "[^"]*"/m, `version = "${version}"`));
}

function updatePackageLockJson(relativePath) {
  const path = resolve(root, relativePath);
  const json = JSON.parse(readFileSync(path, "utf8"));
  json.version = version;
  if (json.packages?.[""]) json.packages[""].version = version;
  writeFileSync(path, JSON.stringify(json, null, 2) + "\n");
}

async function totalDownloads() {
  try {
    const res = await fetch("https://api.github.com/repos/kua8/Nectar/releases?per_page=100");
    if (!res.ok) return null;
    const releases = await res.json();
    return releases.reduce((sum, r) => sum + r.assets.reduce((n, a) => n + a.download_count, 0), 0);
  } catch {
    return null;
  }
}

function formatCount(n) {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return String(n);
}

async function updateReadmeBadges() {
  const path = resolve(root, "README.md");
  let content = readFileSync(path, "utf8");
  content = content.replace(/shieldcn\.dev\/badge\/release-v[\d.]+\.svg/, `shieldcn.dev/badge/release-v${version}.svg`);
  const total = await totalDownloads();
  if (total !== null) {
    const downloads = /shieldcn\.dev\/(?:github\/downloads\/kua8\/Nectar|badge\/downloads-[\d.]+[kM]?)\.svg\?logo=lu:Download(?:&v=[^)&]*)?/;
    content = content.replace(downloads, `shieldcn.dev/badge/downloads-${formatCount(total)}.svg?logo=lu:Download`);
  }
  writeFileSync(path, content);
}

updateJson("package.json");
updateJson("src-tauri/tauri.conf.json");
updateCargoToml();
updateCargoLock("src-tauri/Cargo.lock", "nectar");
updatePackageLockJson("package-lock.json");

updateJson("installer-app/package.json");
updateJson("installer-app/src-tauri/tauri.conf.json");
updateInstallerCargoToml();
updateCargoLock("installer-app/src-tauri/Cargo.lock", "nectar-installer");
updatePackageLockJson("installer-app/package-lock.json");
await updateReadmeBadges();

console.log(`Bumped package.json, tauri.conf.json, Cargo.toml, Cargo.lock and package-lock.json (main app + installer-app) to ${version}`);
