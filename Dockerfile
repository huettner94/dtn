####################################################################################################
## Builder
####################################################################################################
FROM rust:1.69 AS builder

RUN rustup target add x86_64-unknown-linux-musl
RUN apt update && apt install -y musl-tools musl-dev git
RUN update-ca-certificates

# Create appuser
ENV USER=dtrd
ENV UID=10001

RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    "${USER}"


WORKDIR /dtrd

COPY ./ .

# as a workaround to make registry updates faster
RUN mkdir -p ~/.cargo/
RUN echo "[net]" > ~/.cargo/config.toml
RUN echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml
RUN echo "[registries.crates-io]" >> ~/.cargo/config.toml
RUN echo 'protocol = "sparse"' >> ~/.cargo/config.toml

RUN rustup component add rustfmt
RUN cargo build --release

####################################################################################################
## Final image
####################################################################################################
FROM rust:1.69

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /dtrd

# Copy our build
COPY --from=builder /dtrd/target/release/dtrd ./

# Use an unprivileged user.
USER dtrd:dtrd

CMD ["/dtrd/dtrd"]
