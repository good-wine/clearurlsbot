# --- Build Stage ---
FROM docker.io/rust:1.92-slim-bookworm as builder

# Optimization: install dependencies separately to cache layers
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/app
COPY . .

# Build the release binary with optimized settings
RUN cargo build --release --bin clear_urls_bot

# --- Runtime Stage ---
FROM docker.io/debian:bookworm-slim

# Create a non-root user for security (Podman compatible)
RUN groupadd -r clearurls && useradd -r -g clearurls clearurls

RUN apt-get update && apt-get install -y \
    libssl3 \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy assets and binary
COPY --from=builder /usr/src/app/target/release/clear_urls_bot /usr/local/bin/clear_urls_bot
COPY --from=builder /usr/src/app/templates ./templates

# Ensure the database file can be written if using SQLite
RUN touch bot.db && chown clearurls:clearurls bot.db && chown -R clearurls:clearurls /app

USER clearurls

EXPOSE 3000

ENV APP_ENV=production
ENV RUST_LOG=clear_urls_bot=info

CMD ["clear_urls_bot"]