FROM rust:latest as builder
ENV APP_NAME=sol-quic

# build
WORKDIR /usr/src/${APP_NAME}
COPY . .
RUN cargo install --path .

# execute
FROM debian:stable-slim
COPY --from=builder /usr/local/cargo/bin/${APP_NAME} /usr/local/bin/${APP_NAME}
CMD ${APP_NAME}