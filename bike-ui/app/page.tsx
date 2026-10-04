import ActivityStream from "../components/ActivityStream";
import Layout from "../components/Layout";
import RequireAuth from "../components/RequireAuth";

export default function HomePage() {
  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-6xl flex-col gap-8 px-4 py-8 sm:px-6 lg:px-8">
        <RequireAuth>
          <ActivityStream />
        </RequireAuth>
      </div>
    </Layout>
  );
}
