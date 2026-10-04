import { copyFileSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const root = new URL("../", import.meta.url);
const inventory = JSON.parse(
  readFileSync(new URL("docs/shared-assets.json", root), "utf8"),
);
for (const asset of inventory.assets) {
  for (const mirror of asset.mirrors) {
    copyFileSync(
      fileURLToPath(new URL(asset.canonical, root)),
      fileURLToPath(new URL(mirror, root)),
    );
    process.stdout.write(mirror + "\n");
  }
}
