ARG K6_IMAGE
FROM ${K6_IMAGE:-scratch}
RUN ["k6", "version"]
COPY smoke.js /checks/smoke.js
ENTRYPOINT ["k6", "run", "/checks/smoke.js"]
