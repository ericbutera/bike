import DhGoalsProgressPanel from "../../components/DhGoalsProgressPanel";
import Layout from "../../components/Layout";

export default function DhGoalsPage() {
  return (
    <Layout>
      <div className="mx-auto flex w-full max-w-7xl flex-col gap-8 px-4 py-8 sm:px-6 lg:px-8">
        <DhGoalsProgressPanel />
      </div>
    </Layout>
  );
}
