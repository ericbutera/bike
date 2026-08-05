import Link from "next/link";

export default function AuthRequiredCard() {
  return (
    <section className="card overflow-hidden bg-base-100 shadow-xl">
      <figure className="bg-base-300">
        <img
          src="/social-preview.jpg"
          alt="all vibes bike analytic platform"
          className="max-h-[32rem] w-full object-cover"
        />
      </figure>
      <div className="card-body">
        <h2 className="card-title text-3xl">all vibes bike analytic platform</h2>
        <p className="max-w-2xl text-base-content/70">
          Sign in to view activities, training progress, and account details.
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
