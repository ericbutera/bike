import { randomUUID } from "node:crypto";
import {
  mkdir,
  readFile,
  readdir,
  rename,
  stat,
  unlink,
  utimes,
  writeFile,
} from "node:fs/promises";
import { join } from "node:path";

const defaultTtlSeconds = 7 * 24 * 60 * 60;

export function cacheTtlMs(value = process.env.MAP_IMAGE_CACHE_TTL_SECONDS) {
  if (value === undefined) return defaultTtlSeconds * 1000;
  const seconds = Number(value);
  if (!Number.isSafeInteger(seconds) || seconds <= 0) {
    throw new Error("MAP_IMAGE_CACHE_TTL_SECONDS must be a positive integer");
  }
  return seconds * 1000;
}

export function createImageCache(directory, ttlMs = cacheTtlMs()) {
  const expired = (mtimeMs) => Date.now() - mtimeMs >= ttlMs;
  const file = (key) => join(directory, `${key}.png`);

  return {
    async init() {
      await mkdir(directory, { recursive: true });
    },
    async read(key) {
      const path = file(key);
      try {
        if (expired((await stat(path)).mtimeMs)) return null;
        const image = await readFile(path);
        const now = new Date();
        await utimes(path, now, now);
        return image;
      } catch (error) {
        if (error.code === "ENOENT") return null;
        throw error;
      }
    },
    async write(key, png) {
      const path = file(key);
      const temporary = `${path}.tmp-${randomUUID()}`;
      try {
        await writeFile(temporary, png);
        await rename(temporary, path);
      } finally {
        await unlink(temporary).catch((error) => {
          if (error.code !== "ENOENT") throw error;
        });
      }
    },
    async prune() {
      let deleted = 0;
      for (const entry of await readdir(directory, { withFileTypes: true })) {
        if (
          !entry.isFile() ||
          !/^[a-f0-9]{64}\.png(?:\.tmp-[a-f0-9-]+)?$/.test(entry.name)
        )
          continue;
        const path = join(directory, entry.name);
        try {
          if (expired((await stat(path)).mtimeMs)) {
            await unlink(path);
            deleted++;
          }
        } catch (error) {
          if (error.code !== "ENOENT") throw error;
        }
      }
      return deleted;
    },
    async stats() {
      let images = 0;
      let bytes = 0;
      for (const entry of await readdir(directory, { withFileTypes: true })) {
        if (!entry.isFile()) continue;
        try {
          const info = await stat(join(directory, entry.name));
          bytes += info.size;
          if (/^[a-f0-9]{64}\.png$/.test(entry.name)) images++;
        } catch (error) {
          if (error.code !== "ENOENT") throw error;
        }
      }
      return { images, bytes };
    },
  };
}
