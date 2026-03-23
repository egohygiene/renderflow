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

# Install Rust via rustup (stable toolchain)
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --no-modify-path --default-toolchain stable && \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME

# Install additional Cargo tooling
RUN cargo install cross --locked && \
    cargo install cargo-watch --locked

# Install Taskfile (go-task) – pinned to v3.43.3, downloaded directly from GitHub releases
ARG TASK_VERSION=v3.43.3
RUN curl --proto '=https' --tlsv1.2 -fsSL \
    "https://github.com/go-task/task/releases/download/${TASK_VERSION}/task_linux_amd64.tar.gz" \
    -o /tmp/task.tar.gz && \
    tar -xzf /tmp/task.tar.gz -C /usr/local/bin task && \
    rm /tmp/task.tar.gz && \
    chmod +x /usr/local/bin/task

# Ensure cargo bin is on PATH for all users / shells
RUN echo 'export PATH="/usr/local/cargo/bin:$PATH"' >> /etc/bash.bashrc

WORKDIR /workspaces/renderflow

SHELL ["/bin/bash", "-c"]
