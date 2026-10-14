import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { afterEach, describe, expect, it } from "vitest";

const fixtures: string[] = [];

afterEach(() => {
  for (const fixture of fixtures.splice(0)) {
    rmSync(fixture, { recursive: true, force: true });
  }
});

function runImageWorkflow(event: string, validationExit = "0") {
  const fixture = mkdtempSync(join(tmpdir(), "bike-ci-images-"));
  fixtures.push(fixture);
  const scripts = join(fixture, "scripts");
  const binaries = join(fixture, "bin");
  const temporary = join(fixture, "tmp");
  for (const directory of [scripts, binaries, temporary]) {
    mkdirSync(directory);
  }
  const calls = join(fixture, "calls.log");
  copyFileSync(
    resolve("../scripts/ci-images.sh"),
    join(scripts, "ci-images.sh"),
  );
  writeFileSync(
    join(binaries, "mise"),
    `#!/usr/bin/env bash
set -euo pipefail
printf '%s|%s\\n' "$*" "$IMAGE_REGISTRY" >>"$CALL_LOG"
if [[ "$2" == images:validate ]]; then exit "$VALIDATION_EXIT"; fi
`,
    { mode: 0o755 },
  );
  writeFileSync(
    join(binaries, "docker-cli-plugin-docker-buildx"),
    '#!/usr/bin/env bash\nprintf "%s\\n" "$*" >>"$CALL_LOG"\n',
    { mode: 0o755 },
  );
  const result = spawnSync("bash", [join(scripts, "ci-images.sh")], {
    cwd: fixture,
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${binaries}:${process.env.PATH}`,
      TMPDIR: temporary,
      CALL_LOG: calls,
      VALIDATION_EXIT: validationExit,
      CI_COMMIT_SHA: "a".repeat(40),
      CI_PIPELINE_NUMBER: "fixture",
      CI_PIPELINE_EVENT: event,
      CI_COMMIT_PULL_REQUEST: "42",
      CI_COMMIT_BRANCH: "main",
      CI_COMMIT_SOURCE_BRANCH: "main",
      BIKE_BUILDKIT_ADDR: "tcp://fixture:1234",
      BIKE_BUILDKIT_CA: "fixture-ca",
      BIKE_BUILDKIT_CERT: "fixture-cert",
      BIKE_BUILDKIT_KEY: "fixture-key",
    },
  });
  return {
    ...result,
    calls: readFileSync(calls, "utf8").split("\n"),
    temporary: readdirSync(temporary),
  };
}

describe("CI image workflow", () => {
  it.each(["push", "pull_request"])(
    "scopes validation destinations and publishes %s release images",
    (event) => {
      const result = runImageWorkflow(event);
      expect(result.stderr).toBe("");
      expect(result.status).toBe(0);
      expect(result.calls).toContain("run images:validate|");
      expect(result.calls).toContain("run images:push|registry.registry:5000");
      expect(result.calls).toContain("rm bike-ci-fixture");
      expect(result.temporary).toEqual([]);
    },
  );

  it("does not publish after validation fails and removes its client credentials", () => {
    const result = runImageWorkflow("pull_request", "9");
    expect(result.status).toBe(9);
    expect(
      result.calls.some((call) => call.startsWith("run images:push")),
    ).toBe(false);
    expect(result.calls).toContain("rm bike-ci-fixture");
    expect(result.temporary).toEqual([]);
  });
});
