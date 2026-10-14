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

function runImageWorkflow(
  event: string,
  validationExit = "0",
  publication = "published",
) {
  const fixture = mkdtempSync(join(tmpdir(), "bike-ci-images-"));
  fixtures.push(fixture);
  const scripts = join(fixture, "scripts");
  const binaries = join(fixture, "bin");
  const temporary = join(fixture, "tmp");
  for (const directory of [scripts, binaries, temporary]) {
    mkdirSync(directory);
  }
  const calls = join(fixture, "calls.log");
  const targets = ["api", "worker", "ui", "maps", "e2e"];
  const plan = join(fixture, "plan.json");
  const metadata = join(fixture, "metadata.json");
  writeFileSync(
    plan,
    JSON.stringify({
      target: Object.fromEntries(
        targets.map((target) => [
          target,
          {
            tags: [`registry.registry:5000/bike-${target}:${"a".repeat(40)}`],
            output: [
              {
                type: "image",
                push: publication === "disabled-push" ? "false" : "true",
              },
            ],
          },
        ]),
      ),
    }),
  );
  writeFileSync(
    metadata,
    JSON.stringify(
      Object.fromEntries(
        targets.map((target, index) => [
          target,
          {
            "containerimage.digest":
              publication === "missing-metadata"
                ? null
                : `sha256:${String(index + 1).repeat(64)}`,
          },
        ]),
      ),
    ),
  );
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
if [[ "$*" == 'run images:push -- --print' ]]; then cat "$PUBLICATION_PLAN"; exit 0; fi
if [[ "$2" == images:push ]]; then cp "$PUBLICATION_METADATA" .artifacts/image-builds.json; fi
`,
    { mode: 0o755 },
  );
  writeFileSync(
    join(binaries, "curl"),
    `#!/usr/bin/env bash
set -euo pipefail
printf 'curl %s\\n' "$*" >>"$CALL_LOG"
url="${"${!#}"}"
image="${"${url#*/v2/bike-}"}"
target="${"${image%%/*}"}"
if [[ "$target" == e2e && "$PUBLICATION_CASE" == missing-tag ]]; then exit 22; fi
digest="$(jq -r --arg target "$target" '.[$target]["containerimage.digest"]' "$PUBLICATION_METADATA")"
if [[ "$PUBLICATION_CASE" == wrong-digest ]]; then digest="sha256:${"f".repeat(64)}"; fi
printf 'HTTP/1.1 200 OK\\r\\nDocker-Content-Digest: %s\\r\\n\\r\\n' "$digest"
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
      PUBLICATION_PLAN: plan,
      PUBLICATION_METADATA: metadata,
      PUBLICATION_CASE: publication,
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
      expect(
        result.calls.filter((call) => call.startsWith("curl ")),
      ).toHaveLength(5);
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

  it("rejects a plan that can omit publication before building", () => {
    const result = runImageWorkflow("push", "0", "disabled-push");
    expect(result.status).not.toBe(0);
    expect(result.stderr).toContain(
      "Image publication plan must push every target",
    );
    expect(result.calls).not.toContain(
      "run images:push|registry.registry:5000",
    );
    expect(result.temporary).toEqual([]);
  });

  it.each(["missing-tag", "wrong-digest", "missing-metadata"])(
    "fails publication verification for %s and removes credentials",
    (publication) => {
      const result = runImageWorkflow("push", "0", publication);
      expect(result.status).not.toBe(0);
      expect(result.calls).toContain("rm bike-ci-fixture");
      expect(result.temporary).toEqual([]);
    },
  );
});
