FROM rust:latest AS chef
RUN cargo install cargo-chef
WORKDIR /app

FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM chef as builder
COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release -p pwnhub-bot --recipe-path recipe.json

COPY . .
ARG SQLX_OFFLINE=true
RUN cargo build --release -p pwnhub-bot

FROM debian:buster-slim AS runtime
WORKDIR /app
COPY --from=builder /app/target/release/pwnhub-bot /usr/local/bin
ENTRYPOINT [ "/usr/local/bin/pwnhub-bot" ]