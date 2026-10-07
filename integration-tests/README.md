# Synthetic availability check

The k6 image makes two read-only requests: API health and the UI HTML page.
It needs no user, fixture data, authentication, Kubernetes CLI, or port forwards.
It runs once and exits nonzero when either check fails. A sidecar scheduler or
monitoring job inside the VPC can invoke the same image periodically.

```sh
mise run synthetics:build
BIKE_API_URL=http://api:3000/api BIKE_UI_URL=http://ui:3000 mise run synthetics:run
```

Both URLs must be reachable from the container. Locally, use
`host.docker.internal` for host services or attach the image to the application's
Docker network. Inside Kubernetes use service DNS names or localhost when the
checked service shares the pod. Inject configuration when launching the image;
the runner never queries cluster configuration.

Woodpecker builds this image and runs its entrypoint inside the cluster after
an authorized deployment. Pull requests build and validate it without contacting
production. `mise run synthetics:check` validates the k6 configuration.
The k6 version is pinned in root mise vars and passed by `synthetics:build`;
the Dockerfile default is also maintained for standalone CI builds.

[Browser e2e tests](../bike-ui/tests/e2e/README.md) have a separate image and
cover activity/segment navigation and race playback. Native HTTP integration
tests remain in `bike-rs/api/tests`. They are separate from availability monitoring.
