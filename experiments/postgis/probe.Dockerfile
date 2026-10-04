FROM rust@sha256:80302520b7199f0504975bca59a914015e9fee088f759875dbbc238ca9509ee1 AS build
WORKDIR /work/experiments/postgis/probe
COPY experiments/postgis/probe ./
COPY bike-rs/bike-core/src/segment_support.rs /work/bike-rs/bike-core/src/segment_support.rs
COPY bike-rs/bike-core/src/activity_data.rs /work/bike-rs/bike-core/src/activity_data.rs
COPY bike-rs/bike-core/src/activity_sport.rs /work/bike-rs/bike-core/src/activity_sport.rs
RUN cargo test --locked && cargo build --locked --release

FROM debian@sha256:a99cfc517144bc59b1978475ec53b46ecabec7e43635402ee5b77cc54cd1b20a
COPY --from=build /work/experiments/postgis/probe/target/release/postgis-probe /usr/local/bin/postgis-probe
ENTRYPOINT ["postgis-probe"]
