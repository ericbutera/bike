import assert from "node:assert/strict";
import { test } from "node:test";
import { imageKey, parseRenderRequest } from "./request.mjs";

const points = [
  { latitude: 44.7631, longitude: -85.6206 },
  { latitude: 44.769, longitude: -85.59 },
];

test("identical geometry from separate sites shares an image key", () => {
  const first = parseRenderRequest({
    points,
    theme: "light",
    variant: "thumbnail",
    dpr: 1,
    profile: "shared",
    scope: "first-login",
    activityId: 10,
  });
  const second = parseRenderRequest({
    points: points.map((point) => ({ ...point, elapsed_seconds: 12 })),
    theme: "light",
    variant: "thumbnail",
    dpr: 1,
    profile: "shared",
    scope: "second-login",
    activityId: 22,
  });
  assert.deepEqual(first, second);
  assert.equal(imageKey(first), imageKey(second));
  const oldRustProfile = parseRenderRequest({
    points,
    theme: "light",
    variant: "thumbnail",
    dpr: 1,
    profile: "rust",
  });
  assert.equal(imageKey(first), imageKey(oldRustProfile));
});

test("invalid coordinates and rendering options are rejected", () => {
  const base = {
    points,
    theme: "light",
    variant: "full",
    dpr: 1,
  };
  assert.equal(
    parseRenderRequest({ ...base, points: points.slice(0, 1) }),
    null,
  );
  assert.equal(
    parseRenderRequest({
      ...base,
      points: [{ ...points[0], latitude: 91 }, points[1]],
    }),
    null,
  );
});
