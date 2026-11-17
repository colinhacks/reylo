# The reylo command as a container image, ghcr.io/reylosql/reylo: the static musl build of each CPU
# over distroless's static base, which adds CA certificates, time zones and nothing else. Its context
# is a directory holding `<arch>/reylo` for amd64 and arm64, unpacked from a release's
# `reylo-linux-<cpu>-musl.tar.gz` (release.yml, channels.yml).
#
#   docker run --rm -v "$PWD:/work" ghcr.io/reylosql/reylo migrate status
FROM gcr.io/distroless/static-debian12@sha256:d75cdd72874d4790092fcb1b058493ecf6bb5bf2b2b897045b00ff01d91843f2
ARG TARGETARCH
ARG VERSION
LABEL org.opencontainers.image.source="https://github.com/reylosql/reylo" \
      org.opencontainers.image.url="https://reylo.dev" \
      org.opencontainers.image.description="The reylo command: infer types for SQL queries from the database itself, and run migrations" \
      org.opencontainers.image.licenses="MIT" \
      org.opencontainers.image.version="${VERSION}"
COPY --chmod=755 ${TARGETARCH}/reylo /usr/local/bin/reylo
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/reylo"]
