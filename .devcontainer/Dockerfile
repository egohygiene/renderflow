FROM mcr.microsoft.com/devcontainers/base:ubuntu

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    pandoc \
    curl \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Install tectonic PDF engine – pinned to v0.15.0, downloaded from official releases
ARG TECTONIC_VERSION=0.15.0
RUN curl --proto '=https' --tlsv1.2 -fsSL \
    "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    -o /tmp/tectonic.tar.gz && \
    tar -xzf /tmp/tectonic.tar.gz -C /usr/local/bin tectonic && \
    rm /tmp/tectonic.tar.gz && \
    chmod +x /usr/local/bin/tectonic

# Install Taskfile (go-task) – pinned to v3.43.3, downloaded directly from GitHub releases
ARG TASK_VERSION=v3.43.3
RUN curl --proto '=https' --tlsv1.2 -fsSL \
    "https://github.com/go-task/task/releases/download/${TASK_VERSION}/task_linux_amd64.tar.gz" \
    -o /tmp/task.tar.gz && \
    tar -xzf /tmp/task.tar.gz -C /usr/local/bin task && \
    rm /tmp/task.tar.gz && \
    chmod +x /usr/local/bin/task

# Install git-cliff – pinned to v2.12.0, downloaded directly from GitHub releases
ARG GIT_CLIFF_VERSION=v2.12.0
RUN curl --proto '=https' --tlsv1.2 -fsSL \
    "https://github.com/orhun/git-cliff/releases/download/${GIT_CLIFF_VERSION}/git-cliff-${GIT_CLIFF_VERSION#v}-x86_64-unknown-linux-musl.tar.gz" \
    -o /tmp/git-cliff.tar.gz && \
    tar -xzf /tmp/git-cliff.tar.gz -C /tmp "git-cliff-${GIT_CLIFF_VERSION#v}/git-cliff" && \
    mv "/tmp/git-cliff-${GIT_CLIFF_VERSION#v}/git-cliff" /usr/local/bin/git-cliff && \
    rm -rf /tmp/git-cliff.tar.gz "/tmp/git-cliff-${GIT_CLIFF_VERSION#v}" && \
    chmod +x /usr/local/bin/git-cliff

# Switch to the devcontainer non-root user and install Rust in the user's home directory
USER vscode

ENV RUSTUP_HOME=/home/vscode/.rustup \
    CARGO_HOME=/home/vscode/.cargo \
    PATH=/home/vscode/.cargo/bin:$PATH

# Install Rust via rustup (stable toolchain)
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --no-modify-path --default-toolchain stable

# Install additional Cargo tooling
RUN cargo install cross --locked && \
    cargo install cargo-watch --locked

WORKDIR /workspaces/renderflow

SHELL ["/bin/bash", "-c"]
