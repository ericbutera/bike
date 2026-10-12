import { lstatSync, readdirSync } from "node:fs";
import { join } from "node:path";

function readDuringBuild(read) {
  try {
    return read();
  } catch (error) {
    // Cargo can unlink a file or directory after its parent was listed.
    if (error.code === "ENOENT" || error.code === "ENOTDIR") return null;
    throw error;
  }
}

function allocatedBytes(root) {
  const device = lstatSync(root, { bigint: true }).dev;
  const pending = [root];
  const seen = new Set();
  let bytes = 0n;
  while (pending.length) {
    const path = pending.pop();
    const stat = readDuringBuild(() => lstatSync(path, { bigint: true }));
    if (!stat || stat.dev !== device) continue;
    const identity = `${stat.dev}:${stat.ino}`;
    if (seen.has(identity)) continue;
    seen.add(identity);
    bytes += stat.blocks * 512n;
    if (stat.isDirectory()) {
      const entries = readDuringBuild(() => readdirSync(path)) ?? [];
      for (const name of entries) pending.push(join(path, name));
    }
  }
  return bytes;
}

process.stdout.write(`${allocatedBytes(process.argv[2])}\n`);
