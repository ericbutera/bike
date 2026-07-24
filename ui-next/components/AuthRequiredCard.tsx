import Link from "next/link";

export default function AuthRequiredCard({
  eyebrow,
  title,
  description,
  loginLabel = "Sign in",
}: {
  eyebrow?: string;
  title: string;
  description: string;
  loginLabel?: string;
}) {
  return (
    <section className="card bg-base-100 shadow-xl">
      <div className="card-body">
        {eyebrow ? (
          <p className="text-sm text-base-content/60">{eyebrow}</p>
        ) : null}
        <h2 className="card-title text-3xl">{title}</h2>
        <p className="max-w-2xl text-base-content/70">{description}</p>
        <div className="card-actions justify-start">
          <Link href="/login" className="btn btn-primary">
            {loginLabel}
          </Link>
        </div>
      </div>
    </section>
  );
}
