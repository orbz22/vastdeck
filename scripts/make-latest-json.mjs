// Builds the update manifest the in-app updater reads.
//
// Tauri writes a `.sig` next to each installer when TAURI_SIGNING_PRIVATE_KEY is
// set during the build. That signature is what the installed app checks before
// it will run a downloaded installer, so a build made without the key produces
// no manifest and simply cannot ship as an update.
//
// Run after `npm run app:build`, then upload latest.json with the release.
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const { version } = JSON.parse(
  fs.readFileSync(path.join(root, "package.json"), "utf8"),
);

const repo = "orbz22/vastdeck";
const nsisDir = path.join(root, "src-tauri", "target", "release", "bundle", "nsis");
const installer = `Vastdeck_${version}_x64-setup.exe`;
const sigPath = path.join(nsisDir, `${installer}.sig`);

if (!fs.existsSync(path.join(nsisDir, installer))) {
  console.error(`No installer at ${path.join(nsisDir, installer)}. Run \`npm run app:build\` first.`);
  process.exit(1);
}
if (!fs.existsSync(sigPath)) {
  console.error(
    `No signature at ${sigPath}.\n` +
      "The build ran without TAURI_SIGNING_PRIVATE_KEY, so this build cannot be\n" +
      "published as an update. Re-run the build with the key set.",
  );
  process.exit(1);
}

// Release notes come from the same file the GitHub release uses, first paragraph
// only — the updater shows this in a small box, not a full changelog.
const notesPath = path.join(root, "RELEASE_NOTES.md");
const notes = fs.existsSync(notesPath)
  ? fs.readFileSync(notesPath, "utf8").split("\n\n")[0].trim()
  : `Vastdeck ${version}`;

const manifest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": {
      signature: fs.readFileSync(sigPath, "utf8").trim(),
      url: `https://github.com/${repo}/releases/download/v${version}/${installer}`,
    },
  },
};

const out = path.join(root, "src-tauri", "target", "release", "bundle", "latest.json");
fs.writeFileSync(out, JSON.stringify(manifest, null, 2) + "\n");
console.log(`${path.relative(root, out)}  ->  ${manifest.platforms["windows-x86_64"].url}`);
