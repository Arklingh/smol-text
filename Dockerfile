# --- Stage 1: Build the binary ---
FROM rust:1.85-slim AS builder

WORKDIR /app

# Встановлюємо необхідні системні бібліотеки для збірки (включаючи OpenSSL, якщо потрібен)
RUN apt-get update && apt-get install -y pkg-config libssl-dev & rm -rf /var/lib/apt/lists/*

# Копіюємо конфіги
COPY Cargo.toml Cargo.lock ./

# Створюємо пустий файл src/main.rs, щоб закешувати залежності
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -f target/release/deps/smol_text*

# Тепер копіюємо реальний код та статичні файли
COPY src ./src
COPY static ./static

# Збираємо фінальний релізний бінарник
RUN cargo build --release

# --- Stage 2: Run the binary ---
FROM debian:bookworm-slim

WORKDIR /app

# Інсталюємо сертифікати, щоб наш сервіс міг робити HTTP-запити (наприклад, до QR-генератора)
RUN apt-get update && apt-get install -y ca-certificates & rm -rf /var/lib/apt/lists/*

# Копіюємо бінарник та папку static зі Stage 1
COPY --from=builder /app/target/release/smol-text ./smol-text
COPY --from=builder /app/static ./static

# Вказуємо команду для запуску
CMD ["./smol-text"]
