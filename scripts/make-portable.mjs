// Packages the release binary as a portable zip: the executable, the marker
// file that switches it to storing data beside itself, and the licence.
//
// Run after `npm run app:build`.
import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const version = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
).version;

const exe = path.join(root, "src-tauri", "target", "release", "vastdeck.exe");
if (!fs.existsSync(exe)) {
  console.error("No release binary found. Run `npm run app:build` first.");
  process.exit(1);
}

const outDir = path.join(root, "src-tauri", "target", "release", "bundle", "portable");
const stage = path.join(outDir, `Vastdeck_${version}_x64_portable`);
fs.rmSync(stage, { recursive: true, force: true });
fs.mkdirSync(stage, { recursive: true });

fs.copyFileSync(exe, path.join(stage, "vastdeck.exe"));
fs.copyFileSync(path.join(root, "LICENSE"), path.join(stage, "LICENSE"));
fs.writeFileSync(
  path.join(stage, "portable.txt"),
  [
    "This file switches Vastdeck to portable mode.",
    "",
    "Settings, the session cache and launch scripts are kept in the data\\",
    "folder beside vastdeck.exe instead of in %LOCALAPPDATA%. Move or delete",
    "this folder and nothing of Vastdeck's is left behind.",
    "",
    "Delete this file to use %LOCALAPPDATA% like the installed build does.",
    "",
  ].join("\r\n"),
);

const zip = `${stage}.zip`;
fs.rmSync(zip, { force: true });
// Compress-Archive ships with Windows, so nothing extra has to be installed.
execFileSync("powershell", [
  "-NoProfile",
  "-Command",
  `Compress-Archive -Path '${stage}\\*' -DestinationPath '${zip}' -Force`,
]);
fs.rmSync(stage, { recursive: true, force: true });

const size = (fs.statSync(zip).size / 1024 / 1024).toFixed(1);
console.log(`${path.relative(root, zip)}  (${size} MB)`);
