FROM ubuntu:22.04

# Avoid interactive prompts
ENV DEBIAN_FRONTEND=noninteractive

# Install dependencies
RUN apt-get update -qq && \
    apt-get install -y -qq \
    curl \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Install Rust toolchain first
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y

# Install Solana CLI
RUN sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"

# Set PATH to include Rust and Solana tools
ENV PATH="/root/.cargo/bin:/root/.local/share/solana/install/active_release/bin:${PATH}"

# Create cargo-build-bpf wrapper (Anchor 0.29.0 expects build-bpf, newer Solana uses build-sbf)
# When Cargo calls cargo-build-bpf, it passes "build-bpf" as first arg, but cargo-build-sbf doesn't expect it
# So we skip the first argument if it's "build-bpf" and pass the rest to cargo-build-sbf
RUN mkdir -p /root/.cargo/bin && \
    echo '#!/bin/bash' > /root/.cargo/bin/cargo-build-bpf && \
    echo 'if [ "$1" = "build-bpf" ]; then shift; fi' >> /root/.cargo/bin/cargo-build-bpf && \
    echo 'exec cargo-build-sbf "$@"' >> /root/.cargo/bin/cargo-build-bpf && \
    chmod +x /root/.cargo/bin/cargo-build-bpf

# Install Rust 1.76.0 (compatible with both Solana and newer dependencies)
RUN rustup toolchain install 1.76.0 && rustup default 1.76.0

# Verify cargo is available and version
RUN cargo --version && rustc --version

# Install Anchor 0.29.0 with --locked (required version for our project)
RUN cargo install --git https://github.com/coral-xyz/anchor --tag v0.29.0 anchor-cli --locked

# Set working directory
WORKDIR /workspace

# Copy project files
COPY . .

# Build command (will be overridden)
CMD ["anchor", "build"]

