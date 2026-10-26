FROM rust@sha256:8fa55b2f3ddf97471ab6a767bfa3f37e6bad0986ba823e75fea57e2a2a5c3073

# Native AArch64 Linux builder. The base image pins rustc; Cargo.lock pins Rust deps.
RUN test "$(uname -m)" = aarch64 \
    && apt-get update \
    && apt-get install -y --no-install-recommends \
       musl-tools clang cmake llvm libseccomp-dev gperf libtool autoconf automake \
    && ln -s /usr/include/linux /usr/include/aarch64-linux-musl/linux \
    && ln -s /usr/include/asm-generic /usr/include/aarch64-linux-musl/asm-generic \
    && ln -s /usr/include/aarch64-linux-gnu/asm /usr/include/aarch64-linux-musl/asm \
    && rustup target add aarch64-unknown-linux-musl \
    && rustup component add rustfmt

# Same libseccomp version and musl static-library configuration as upstream.
RUN git clone --depth 1 --branch v2.6.1 https://github.com/seccomp/libseccomp /tmp/libseccomp \
    && cd /tmp/libseccomp \
    && test "$(git rev-parse HEAD)" = a81cc2de8ee7a33633defc88bb8a2d986b421613 \
    && ./autogen.sh \
    && CC='musl-gcc -static' ./configure --enable-static=yes --enable-shared=false \
    && make -j2 && make install \
    && git rev-parse HEAD > /usr/local/lib/libseccomp-revision.txt

WORKDIR /work
