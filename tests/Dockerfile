# The end-to-end suite, with nothing of the host in it. 
# run via tests/sandboxed.sh
FROM ubuntu:26.04

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      bash bash-completion ca-certificates curl gcc libc6-dev python3 \
 && rm -rf /var/lib/apt/lists/*

# install rust
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
  | sh -s -- -y --no-modify-path --profile minimal --default-toolchain stable

WORKDIR /src
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src

# Cached, so an edit to src/ recompiles only what changed.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --lib \
 && mkdir -p /built && cp target/debug/libreedline_bash.so /built/
RUN mkdir -p /src/target/debug \
 && cp /built/libreedline_bash.so /src/target/debug/

COPY tests ./tests

RUN useradd --create-home --home-dir /home/test --uid 1001 test
USER test
ENV HOME=/home/test \
    LC_ALL=C.UTF-8

# Exec form, so the runner is pid 1, so we can determine if we are in the container
ENTRYPOINT ["python3", "tests/e2e.py"]
