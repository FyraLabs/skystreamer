FROM rust:latest
LABEL org.opencontainers.image.source = "https://github.com/FyraLabs/skystreamer"
WORKDIR /usr/src/app
COPY . .

RUN cargo install --path skystreamer-prometheus-exporter

WORKDIR /
RUN rm -rf /usr/src/app


CMD ["skystreamer-prometheus-exporter"]