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
FROM rust:1.92-trixie AS builder

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

RUN apt update

# as a workaround to make registry updates faster
RUN mkdir -p ~/.cargo/
RUN echo "[net]" > ~/.cargo/config.toml
RUN echo "git-fetch-with-cli = true" >> ~/.cargo/config.toml
RUN echo "[registries.crates-io]" >> ~/.cargo/config.toml
RUN echo 'protocol = "sparse"' >> ~/.cargo/config.toml

RUN apt install -y musl-tools musl-dev git protobuf-compiler librocksdb-dev librocksdb9.10 libclang-dev pkg-config libssl-dev
RUN update-ca-certificates

RUN rustup component add rustfmt
RUN ROCKSDB_LIB_DIR=/usr/lib/*-linux-gnu cargo build --release
RUN mkdir /releases
RUN cp /dtrd/target/release/dtrd /dtrd/target/release/dtrd_cli /dtrd/target/release/replistore /releases

####################################################################################################
## Final image
####################################################################################################
FROM rust:1.92-trixie

# Import from builder.
COPY --from=builder /etc/passwd /etc/passwd
COPY --from=builder /etc/group /etc/group

# Install needed libs
RUN apt update && apt install -y librocksdb9.10 libssl-dev

WORKDIR /dtrd

# Copy our build
COPY --from=builder /releases/* ./
ENV PATH="/dtrd:$PATH"

# Use an unprivileged user.
USER dtrd:dtrd

CMD ["/dtrd/dtrd"]
