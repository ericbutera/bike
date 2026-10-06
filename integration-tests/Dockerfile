ARG K6_VERSION=2.3.0
FROM grafana/k6:${K6_VERSION}
COPY smoke.js /checks/smoke.js
ENTRYPOINT ["k6", "run", "/checks/smoke.js"]
