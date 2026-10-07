import assert from "node:assert/strict";
import { mkdtemp, readdir, rm, stat, utimes } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { cacheTtlMs, createImageCache } from "./cache.mjs";

test("cache idle period must be positive", () => {
  assert.equal(cacheTtlMs(undefined), 604800000);
  assert.equal(cacheTtlMs("2"), 2000);
  assert.throws(() => cacheTtlMs("0"));
  assert.throws(() => cacheTtlMs("abc"));
});

test("a request renews retention and idle images are pruned", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bike-map-"));
  const cache = createImageCache(directory, 1000);
  const key = "a".repeat(64);
  const path = join(directory, `${key}.png`);
  try {
    await cache.init();
    await cache.write(key, Buffer.from("image"));
    const old = new Date(Date.now() - 800);
    await utimes(path, old, old);
    assert.equal((await cache.read(key)).toString(), "image");
    assert.ok((await stat(path)).mtimeMs > old.getTime());
    await cache.prune();
    assert.deepEqual(await readdir(directory), [`${key}.png`]);

    await utimes(
      path,
      new Date(Date.now() - 2000),
      new Date(Date.now() - 2000),
    );
    assert.equal(await cache.read(key), null);
    await cache.prune();
    assert.deepEqual(await readdir(directory), []);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
