FROM postgres@sha256:f6ba4a3c3de6ae0f361a8b628d452c14295dc4d076ceb193b8c46fb0a2273e46
ARG POSTGIS_VERSION=3.6.4+dfsg-2.pgdg13+1
RUN apt-get update && apt-get install -y --no-install-recommends \
    postgresql-17-postgis-3=${POSTGIS_VERSION} \
    postgresql-17-postgis-3-scripts=${POSTGIS_VERSION} \
    && rm -rf /var/lib/apt/lists/*
