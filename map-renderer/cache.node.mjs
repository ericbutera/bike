import assert from "node:assert/strict";
import { mkdtemp, readdir, stat, utimes, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { cacheTtlMs, createImageCache } from "./cache.mjs";

test("validates the cache TTL", () => {
  assert.equal(cacheTtlMs(), 604800000);
  assert.equal(cacheTtlMs("2"), 2000);
  assert.throws(() => cacheTtlMs("0"));
  assert.throws(() => cacheTtlMs("abc"));
});

test("serves fresh images and removes expired images", async () => {
  const directory = await mkdtemp(join(tmpdir(), "bike-map-cache-"));
  const cache = createImageCache(directory, 1000);
  const key = "a".repeat(64);
  const path = join(directory, `${key}.png`);
  try {
    await cache.init();
    assert.equal(await cache.read(key), null);
    await cache.write(key, Buffer.from("first"));
    assert.equal((await cache.read(key)).toString(), "first");
    assert.equal(
      (await createImageCache(directory, 1000).read(key)).toString(),
      "first",
    );
    assert.deepEqual(await readdir(directory), [`${key}.png`]);

    const old = new Date(Date.now() - 2000);
    await utimes(path, old, old);
    assert.equal(await cache.read(key), null);
    await cache.prune();
    assert.deepEqual(await readdir(directory), []);

    await cache.write(key, Buffer.from("second"));
    assert.equal((await cache.read(key)).toString(), "second");
    assert.ok((await stat(path)).size > 0);
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});
