import ImportDetail from "../../../components/imports/ImportDetail";

export default async function ImportPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = await params;
  return <ImportDetail importId={id} />;
}
