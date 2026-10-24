import { readFile } from "node:fs/promises";
import { connectedTest as test, expect } from "./helpers/test.mjs";
import { environmentCommand } from "./helpers/environment.mjs";

const fixtures = new URL(
  "../../../bike-rs/tests/fixtures/segment_matching/",
  import.meta.url,
);

async function source(directory, filename) {
  const bytes = await readFile(new URL(`${directory}/${filename}`, fixtures));
  return { filename, contents_hex: bytes.toString("hex") };
}

function candidateIds(pages, limit) {
  for (const page of pages) {
    expect(page.length).toBeGreaterThan(0);
    expect(page.length).toBeLessThanOrEqual(limit);
  }
  const ids = pages.flat();
  expect(ids).toEqual([...new Set(ids)].sort((a, b) => a - b));
  return ids;
}

test("PostgreSQL geographic queries retain the City rides' ordered segment matches", async () => {
  const references = JSON.parse(
    await readFile(new URL("city_efforts.json", fixtures), "utf8"),
  );
  const result = await environmentCommand(
    "seed",
    JSON.stringify({
      records: [],
      segment_matching: {
        user_id: 1,
        activities: [
          { user_id: 1, file: await source("activities", "city_01.fit") },
          { user_id: 2, file: await source("activities", "city_02.fit") },
          { user_id: 1, file: await source("activities", "commons_01.fit") },
        ],
        segments: await Promise.all(
          references.map(({ file }) => source("segments", file)),
        ),
      },
    }),
  );
  // The native fixture command emits one JSON line before the seed wrapper's
  // sequence-reset output. environmentCommand checks diagnostics from all lines.
  const [reportJson] = result.stdout.split("\n");
  const report = JSON.parse(reportJson);
  const segments = new Map(report.segments.map((row) => [row.filename, row]));
  expect(report.activities).toHaveLength(3);
  expect(segments.size).toBe(references.length);
  for (const activity of report.activities.filter((row) =>
    row.filename.startsWith("city_"),
  )) {
    const key = activity.filename === "city_01.fit" ? "city_01" : "city_02";
    const candidates = candidateIds(activity.segment_pages, 3);
    expect(activity.segment_pages.length).toBeGreaterThan(1);
    for (const reference of references) {
      const segment = segments.get(reference.file);
      const activities = candidateIds(segment.activity_pages, 1);
      expect(
        activity.efforts[reference.file],
        `${activity.filename}: ${reference.file}`,
      ).toEqual(reference[key]);
      if (reference[key].length > 0) {
        expect(candidates, `Segment candidate: ${reference.file}`).toContain(
          segment.id,
        );
        expect(
          activities,
          `Activity candidate: ${activity.filename}`,
        ).toContain(activity.id);
      }
    }
  }
  const commons = report.activities.find(
    (row) => row.filename === "commons_01.fit",
  );
  const commonsCandidates = candidateIds(commons.segment_pages, 3);
  for (const file of [
    "Segment - Breakin The Law.gpx",
    "Segment - FMR.gpx",
    "Segment - FMR Full.gpx",
    "Log Jam.gpx",
  ]) {
    const segment = segments.get(file);
    expect(
      commons.efforts[file].length,
      `Commons traversal: ${file}`,
    ).toBeGreaterThan(0);
    expect(commonsCandidates, `Commons segment candidate: ${file}`).toContain(
      segment.id,
    );
    expect(
      candidateIds(segment.activity_pages, 1),
      `Commons activity candidate: ${file}`,
    ).toContain(commons.id);
  }
  expect(
    report.segments.some((segment) => segment.activity_pages.length > 1),
  ).toBe(true);
  expect(report.controls).toHaveLength(4);
  for (const control of report.controls) {
    const limit = control.name.startsWith("activity") ? 1 : 3;
    expect(candidateIds(control.pages, limit), control.name).toEqual(
      control.expected,
    );
  }
});
