import ActivityImportsPanel from "../../components/ActivityImportsPanel";
import Layout from "../../components/Layout";

export default function UploadPage() {
  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-8 px-4 py-8 sm:px-6 lg:px-8">
        <ActivityImportsPanel />
      </div>
    </Layout>
  );
}
