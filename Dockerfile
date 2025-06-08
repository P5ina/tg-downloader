# 🏗 Этап сборки
FROM rust:1.87 AS builder

WORKDIR /app

# Копируем файлы проекта
COPY . .

# Кэшируем зависимости отдельно
RUN cargo fetch

# Собираем release
RUN cargo build --release

# 🏁 Финальный этап
FROM debian:bookworm-slim

# Устанавливаем зависимости: ffmpeg, ca-certificates и curl
RUN apt-get update && apt-get install -y \
    ffmpeg \
    ca-certificates \
    curl \
 && apt-get clean && rm -rf /var/lib/apt/lists/*

# Устанавливаем yt-dlp бинарник
RUN curl -L https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp \
    -o /usr/local/bin/yt-dlp \
 && chmod +x /usr/local/bin/yt-dlp

# Копируем бинарник Rust-приложения из предыдущего этапа
COPY --from=builder /app/target/release/tg-downloader /usr/local/bin/app

VOLUME ["/bot-api-data"]

# По умолчанию запускаем приложение
CMD ["app"]
