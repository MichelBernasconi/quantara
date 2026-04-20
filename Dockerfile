FROM rust:latest AS builder
WORKDIR /usr/src/quantara
COPY . .

# Installa dipendenze per pyo3 e compilazione
RUN apt-get update && apt-get install -y \
    python3 \
    python3-dev \
    libpython3-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

RUN cargo build --release

FROM debian:bookworm-slim
WORKDIR /app

# Installa Python a runtime per far girare gli script
RUN apt-get update && apt-get install -y \
    python3 \
    curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/quantara/target/release/quantara /app/quantara

EXPOSE 3000
CMD ["./quantara"]
