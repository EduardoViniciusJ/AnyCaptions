import { spawnSync } from "node:child_process";
import { readdirSync, rmSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// Run the local CLI from backend/ so Tauri never creates a parallel src-tauri/.
const cli = fileURLToPath(new URL("../node_modules/@tauri-apps/cli/tauri.js", import.meta.url));
const backend = fileURLToPath(new URL("../../backend/", import.meta.url));

// The app writes a `<model>.verified` stamp next to each model it has hashed.
// A stamp only describes this machine's copy, and the installer bundles whole
// model folders, so they are removed before packaging.
function removeVerificationStamps(dir) {
  let entries;
  try {
    entries = readdirSync(dir, { withFileTypes: true });
  } catch {
    return; // No models folder yet: nothing to clean.
  }
  for (const entry of entries) {
    const path = join(dir, entry.name);
    if (entry.isFile() && entry.name.endsWith(".verified")) rmSync(path, { force: true });
    else if (!entry.isFile()) removeVerificationStamps(path);
  }
}
if (process.argv[2] === "build") removeVerificationStamps(join(backend, "models"));

const result = spawnSync(process.execPath, [cli, ...process.argv.slice(2)], {
  cwd: backend,
  stdio: "inherit",
});
if (result.error) {
  console.error("Não foi possível iniciar a CLI do Tauri. Execute npm install em frontend/.");
}
process.exit(result.status ?? 1);
