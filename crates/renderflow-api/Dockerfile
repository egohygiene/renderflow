# Source-only local deployment recipe; build/runtime/license qualification is
# deferred under #420/#419. No image is published by this change.
FROM rust:1.94-bookworm AS build
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
COPY templates ./templates
COPY schemas ./schemas
COPY docs/http-api.openapi.json ./docs/http-api.openapi.json
RUN cargo build --locked --release --package renderflow-api --bin renderflow-api

FROM debian:bookworm-slim
RUN useradd --uid 10001 --create-home --shell /usr/sbin/nologin renderflow \
    && mkdir -p /data/jobs && chown 10001:10001 /data/jobs
COPY --from=build /app/target/release/renderflow-api /usr/local/bin/renderflow-api
COPY LICENSE /usr/share/doc/renderflow/LICENSE
# Retain the source font/template notices carried by the linked core. This is
# not the verified release's complete SBOM/notice/attestation bundle.
COPY --from=build /app/crates/renderflow-core/data /usr/share/renderflow/data
COPY --from=build /app/templates /usr/share/renderflow/templates
USER 10001:10001
ENTRYPOINT ["/usr/local/bin/renderflow-api"]
CMD ["--config", "/config/service.json"]
