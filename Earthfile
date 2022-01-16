VERSION 0.6

# Start with older Ubuntu to ensure GLIBC symbol versioning support for older linux
# Ensure we are using an amd64 platform because some of these targets use cross-platform tooling
FROM --platform amd64 ubuntu:16.04

# Choose where Rust ends up

# Install build prerequisites
deps-base:
    RUN apt-get -y update
    RUN apt-get install -y software-properties-common
    RUN add-apt-repository -y ppa:deadsnakes/ppa
    RUN apt-get -y update
    RUN apt-get install -y iproute2 curl build-essential cmake libssl-dev openssl file git pkg-config python3.8 python3.8-distutils python3.8-dev libdbus-1-dev libdbus-glib-1-dev libgirepository1.0-dev libcairo2-dev 
    RUN apt-get remove -y python3.5
    RUN curl https://bootstrap.pypa.io/get-pip.py | python3.8

# Install Cap'n Proto
deps-capnp:
    FROM +deps-base
    COPY scripts/earthly/install_capnproto.sh /
    RUN /bin/bash /install_capnproto.sh; rm /install_capnproto.sh

# Install Rust
deps-rust:
    FROM +deps-capnp
    ENV RUSTUP_HOME=/usr/local/rustup
    ENV CARGO_HOME=/usr/local/cargo
    ENV PATH=/usr/local/cargo/bin:$PATH
    RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -c clippy --no-modify-path --profile minimal 
    RUN chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
        rustup --version; \
        cargo --version; \
        rustc --version;
    # ARM64 Linux
    RUN rustup target add aarch64-unknown-linux-gnu
    # Android
    RUN rustup target add aarch64-linux-android
    RUN rustup target add armv7-linux-androideabi
    RUN rustup target add i686-linux-android
    RUN rustup target add x86_64-linux-android
    # WASM
    RUN rustup target add wasm32-unknown-unknown

# Install Linux cross-platform tooling
deps-cross:
    FROM +deps-rust
    RUN apt-get install -y gcc-aarch64-linux-gnu curl unzip

# Install android tooling
deps-android:
    FROM +deps-cross
    RUN apt-get install -y openjdk-9-jdk-headless
    RUN mkdir /Android; mkdir /Android/Sdk
    RUN curl -o /Android/cmdline-tools.zip https://dl.google.com/android/repository/commandlinetools-linux-7583922_latest.zip
    RUN cd /Android; unzip /Android/cmdline-tools.zip
    RUN yes | /Android/cmdline-tools/bin/sdkmanager --sdk_root=/Android/Sdk build-tools\;30.0.3 ndk\;22.0.7026061 cmake\;3.18.1 platform-tools platforms\;android-30
    
# Install stub secrets daemon for keyring tests
deps-secretsd:
    FROM +deps-android
    COPY scripts/earthly/secretsd /secretsd
    RUN pip install -r /secretsd/requirements.txt
    RUN pip install keyring
    RUN cp /secretsd/dbus/org.freedesktop.secrets.service /usr/share/dbus-1/services/org.freedesktop.secrets.service

# Clean up the apt cache to save space
deps:
    FROM +deps-secretsd
    RUN apt-get clean

code:
    FROM +deps
    COPY --dir .cargo external files scripts veilid-cli veilid-core veilid-server veilid-wasm Cargo.lock Cargo.toml /veilid
    WORKDIR /veilid

# Clippy only
clippy:
    FROM +code
    RUN cargo clippy

# Build
build-linux-amd64:
    FROM +code
    RUN cargo build --target x86_64-unknown-linux-gnu --release
    SAVE ARTIFACT ./target/x86_64-unknown-linux-gnu AS LOCAL ./target/artifacts/x86_64-unknown-linux-gnu

build-linux-arm64:
    FROM +code
    RUN cargo build --target aarch64-unknown-linux-gnu --release
    SAVE ARTIFACT ./target/aarch64-unknown-linux-gnu AS LOCAL ./target/artifacts/aarch64-unknown-linux-gnu

build-android:
    FROM +code
    WORKDIR /veilid/veilid-core
    ENV PATH=$PATH:/Android/Sdk/ndk/22.0.7026061/toolchains/llvm/prebuilt/linux-x86_64/bin/
    RUN cargo build --target aarch64-linux-android --release
    RUN cargo build --target armv7-linux-androideabi --release
    RUN cargo build --target i686-linux-android --release
    RUN cargo build --target x86_64-linux-android --release
    WORKDIR /veilid
    SAVE ARTIFACT ./target/aarch64-linux-android AS LOCAL ./target/artifacts/aarch64-linux-android
    SAVE ARTIFACT ./target/armv7-linux-androideabi AS LOCAL ./target/artifacts/armv7-linux-androideabi
    SAVE ARTIFACT ./target/i686-linux-android AS LOCAL ./target/artifacts/i686-linux-android
    SAVE ARTIFACT ./target/x86_64-linux-android AS LOCAL ./target/artifacts/x86_64-linux-android

# Unit tests
unit-tests-linux-amd64:
    FROM +code
    RUN cargo test --target x86_64-unknown-linux-gnu --release

unit-tests-linux-arm64:
    FROM +code
    RUN cargo test --target aarch64-unknown-linux-gnu --release