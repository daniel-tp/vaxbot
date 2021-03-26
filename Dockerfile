FROM ekidd/rust-musl-builder:latest as builder

RUN USER=rust cargo new vaxbot
WORKDIR ./vaxbot

ADD --chown=rust:rust . ./
RUN cargo build --release

FROM alpine:latest
RUN apk --no-cache add ca-certificates
COPY --from=builder /home/rust/src/vaxbot/target/x86_64-unknown-linux-musl/release/vaxbot /usr/local/bin/vaxbot

ENTRYPOINT vaxbot