import Link from "next/link";

export default function AuthRequiredCard() {
  return (
    <section className="card bg-base-100 shadow-xl">
      <div className="card-body">
        <h2 className="card-title text-3xl">Sign in required</h2>
        <p className="max-w-2xl text-base-content/70">
          Sign in to view this page.
        </p>
        <div className="card-actions justify-start">
          <Link href="/login" className="btn btn-primary">
            Sign in
          </Link>
        </div>
      </div>
    </section>
  );
}
