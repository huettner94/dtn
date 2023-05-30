####################################################################################################
## Builder
####################################################################################################
FROM --platform=$BUILDPLATFORM rust:1.69 AS builder

ARG TARGETPLATFORM
WORKDIR /dtrd
COPY ./ .

RUN set -exu; \
    apt update;\
    if [ "${TARGETPLATFORM}" = "linux/amd64" ]; then \
    TARGET="x86_64-unknown-linux-gnu"; \
    elif [ "${TARGETPLATFORM}" = "linux/arm64" ]; then \
    TARGET="aarch64-unknown-linux-gnu"; \
    apt install gcc-aarch64-linux-gnu -y; \
    export CARGO_TARGET_AARCH64_UNKOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc; \
    else \
    echo "broken targetplatform"; \
    exit 1; \
    fi; \
    rustup target add "${TARGET}"; \
    apt install -y musl-tools musl-dev git protobuf-compiler;\
    update-ca-certificates; \
    # as a workaround to make registry updates faster
    mkdir -p ~/.cargo/; \
    echo "[net]" > ~/.cargo/config.toml; \
    echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml; \
    echo "[registries.crates-io]" >> ~/.cargo/config.toml; \
    echo 'protocol = "sparse"' >> ~/.cargo/config.toml; \
    rustup component add rustfmt; \
    cargo build --release --target="${TARGET}"; \
    mkdir /releases; \
    cp /dtrd/target/${TARGET}/release/dtrd /releases

####################################################################################################
## Final image
####################################################################################################
FROM rust:1.69

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /dtrd

# Copy our build
COPY --from=builder /releases/dtrd ./

# Use an unprivileged user.
USER dtrd:dtrd

CMD ["/dtrd/dtrd"]
