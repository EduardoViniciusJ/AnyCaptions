// Builds the self-update assets of a release from the installer's own files:
//   anycaptions-update-<version>.zip  the executable and its DLLs, no models
//   update.json                       version, package name, size and SHA-256
//   update.json.minisig               minisign signature of update.json
//
// Run after `npm run tauri build`. The private key stays outside the repository:
//   node scripts/package-update.mjs [--version x.y.z] [--out dir]
// Key path: ANYCAPTIONS_UPDATE_KEY, or ~/.anycaptions/updater.key.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const option = (name) => {
  const index = process.argv.indexOf(`--${name}`);
  return index === -1 ? undefined : process.argv[index + 1];
};
const fail = (message) => {
  console.error(message);
  process.exit(1);
};

const backend = fileURLToPath(new URL("../../backend/", import.meta.url));
const config = JSON.parse(readFileSync(join(backend, "tauri.conf.json"), "utf8"));
const version = option("version") ?? config.version;
const releaseDir = join(backend, "target", "release");
const out = option("out") ?? join(releaseDir, "update-package");
const dllDir = join(backend, "local-data", "nemo-speech", "build-vulkan-asr", "bin");
const keyPath = process.env.ANYCAPTIONS_UPDATE_KEY ?? join(homedir(), ".anycaptions", "updater.key");
const asset = `anycaptions-update-${version}.zip`;

rmSync(out, { recursive: true, force: true });
const staging = join(out, "files");
mkdirSync(staging, { recursive: true });
copyFileSync(join(releaseDir, "anycaptions.exe"), join(staging, "anycaptions.exe"));
const dlls = readdirSync(dllDir).filter((name) => name.toLowerCase().endsWith(".dll"));
if (dlls.length === 0) fail(`No DLLs found in ${dllDir}`);
for (const dll of dlls) copyFileSync(join(dllDir, dll), join(staging, dll));

const zipPath = join(out, asset);
const zipped = spawnSync(
  "powershell",
  [
    "-NoProfile",
    "-Command",
    `Compress-Archive -Path '${join(staging, "*")}' -DestinationPath '${zipPath}' -CompressionLevel Optimal`,
  ],
  { stdio: "inherit" },
);
if (zipped.status !== 0) fail("Could not create the update package.");
rmSync(staging, { recursive: true, force: true });

const bytes = readFileSync(zipPath);
const manifestPath = join(out, "update.json");
writeFileSync(
  manifestPath,
  `${JSON.stringify({
    version,
    asset,
    size: bytes.length,
    sha256: createHash("sha256").update(bytes).digest("hex"),
  })}\n`,
);

const cli = fileURLToPath(new URL("../node_modules/@tauri-apps/cli/tauri.js", import.meta.url));
const signed = spawnSync(
  process.execPath,
  [cli, "signer", "sign", "-f", keyPath, "-p", "", manifestPath],
  { stdio: ["ignore", "ignore", "inherit"] },
);
if (signed.status !== 0) fail(`Could not sign the manifest with ${keyPath}.`);
// The Tauri CLI wraps the minisign signature in base64; the app reads it plain.
const wrapped = join(out, "update.json.sig");
writeFileSync(
  join(out, "update.json.minisig"),
  Buffer.from(readFileSync(wrapped, "utf8").trim(), "base64"),
);
rmSync(wrapped);

console.log(`${asset}: ${(bytes.length / 1024 / 1024).toFixed(1)} MB, ${dlls.length} DLLs`);
console.log(`Upload everything in ${out} to the release.`);
