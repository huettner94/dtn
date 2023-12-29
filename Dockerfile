# Copyright (C) 2023 Felix Huettner
#
# This file is part of DTRD.
#
# DTRD is free software: you can redistribute it and/or modify
# it under the terms of the GNU General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# DTRD is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.
#
# You should have received a copy of the GNU General Public License
# along with this program.  If not, see <https://www.gnu.org/licenses/>.

####################################################################################################
## Builder
####################################################################################################
FROM --platform=$BUILDPLATFORM rust:1.75 AS builder

ARG TARGETPLATFORM
WORKDIR /dtrd
COPY ./ .

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

RUN set -exu
RUN apt update

# as a workaround to make registry updates faster
RUN mkdir -p ~/.cargo/
RUN echo "[net]" > ~/.cargo/config.toml
RUN echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml
RUN echo "[registries.crates-io]" >> ~/.cargo/config.toml
RUN echo 'protocol = "sparse"' >> ~/.cargo/config.toml

# per platform config
RUN if [ "${TARGETPLATFORM}" = "linux/amd64" ]; then \
    TARGET="x86_64-unknown-linux-gnu"; \
    elif [ "${TARGETPLATFORM}" = "linux/arm64" ]; then \
    TARGET="aarch64-unknown-linux-gnu"; \
    apt install gcc-aarch64-linux-gnu -y; \
    export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc; \
    else \
    echo "broken targetplatform"; \
    exit 1; \
    fi; \
    # and now the generic build
    rustup target add "${TARGET}"; \
    apt install -y musl-tools musl-dev git protobuf-compiler;\
    update-ca-certificates; \
    rustup component add rustfmt; \
    cargo build --release --target="${TARGET}"; \
    mkdir /releases; \
    cp /dtrd/target/${TARGET}/release/dtrd /dtrd/target/${TARGET}/release/dtrd_cli /releases

####################################################################################################
## Final image
####################################################################################################
FROM rust:1.75

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

WORKDIR /dtrd

# Copy our build
COPY --from=builder /releases/dtrd /releases/dtrd_cli ./
ENV PATH="/dtrd:$PATH"

# Use an unprivileged user.
USER dtrd:dtrd

CMD ["/dtrd/dtrd"]
