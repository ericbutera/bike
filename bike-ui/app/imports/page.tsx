import ImportHistory from "../../components/imports/ImportHistory";

export default async function ImportsPage({
  searchParams,
}: {
  searchParams: Promise<{ archive_job_id?: string }>;
}) {
  const params = await searchParams;
  const archiveJobId = Number(params.archive_job_id);
  return (
    <ImportHistory
      archiveJobId={
        Number.isInteger(archiveJobId) && archiveJobId > 0
          ? archiveJobId
          : undefined
      }
    />
  );
}
