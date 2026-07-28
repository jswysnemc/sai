# syntax=docker/dockerfile:1.7

# 1. 构建前端静态资源
FROM node:22-bookworm AS web
WORKDIR /src/web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

# 2. 编译 Sai 二进制（嵌入 web/dist）
FROM rust:1-bookworm AS builder
WORKDIR /src

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        libasound2-dev \
        libwayland-dev \
        libxkbcommon-dev \
        pkg-config \
        ripgrep \
    && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock build.rs ./
COPY assets ./assets
COPY sidecars ./sidecars
COPY src ./src
COPY --from=web /src/web/dist ./web/dist

RUN cargo build --release --locked \
    && strip target/release/sai

# 3. 运行时镜像
FROM node:22-bookworm-slim

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        ca-certificates \
        libasound2 \
        libwayland-client0 \
        libxkbcommon0 \
        ripgrep \
        bubblewrap \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /src/target/release/sai /usr/local/bin/sai

# 配置与状态目录可挂载到宿主机
ENV XDG_CONFIG_HOME=/config \
    XDG_STATE_HOME=/state \
    XDG_DATA_HOME=/data \
    XDG_CACHE_HOME=/cache

VOLUME ["/config", "/state", "/data", "/cache", "/workspace"]
WORKDIR /workspace

EXPOSE 4096

ENTRYPOINT ["sai"]
CMD ["--help"]
