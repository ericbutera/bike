ARG K6_IMAGE=grafana/k6:2.3.0@sha256:9c2dee7f8ed74d317e4027c06a10f169b625638189de8d4555d0b3486a5aeb34
FROM ${K6_IMAGE}
COPY smoke.js /checks/smoke.js
ENTRYPOINT ["k6", "run", "/checks/smoke.js"]
