# Branch review environments

- Start a named local environment from a committed Git branch, tag, or SHA.
  Resolve the ref once and record the commit; never switch the editor checkout.
- Build that checkout's API, UI, worker, and renderer using its existing Bake
  targets and mise pins. Keep checkouts and build/cache output on shared storage.
- Each name owns a Compose project, network, ports, uploads, and caches.
  Bind published ports to loopback; use a separate `<name>.localhost` origin.
- Run real PostgreSQL with a size-limited tmpfs data directory. Apply the selected
  branch's migrations and load the controller checkout's shared E2E fixtures.
  All environments launched by that controller use the same fixture definitions.
- Start the app, Temporal dev server, and worker after fixture seeding. Use the
  existing local developer identity; no production session or provider credentials.
- Keep the environment running for manual browsing and Playwright. Print UI,
  API, and Temporal URLs with the resolved revision and fixture scenario.
- Reset recreates and reseeds only the named environment, using its original
  images and saved fixture input. Down also removes its owned checkout and files.
  Never change the regular development database, uploads, or other environments.
- PostgreSQL data disappears when its container stops. Use reset to restore a
  usable fixture environment; these instances are disposable review data.
- Initial scenarios reuse the small synthetic platform dataset. Missing-ranks
  changes only the first activity's stored ranks, allowing old/new reads to be
  compared. The real FIT recording remains a separate fixture.
- Completion: main and a feature branch run simultaneously, open without login,
  show their own revision/data, and survive resetting or removing the other.
  Verify PostgreSQL uses tmpfs and both resets reproduce the seed values.

Remote staging, automatic PR provisioning, database copies from production,
capacity testing, and new worker algorithms are outside this change.
