FROM rust:latest

WORKDIR /usr/src/app
COPY . .

RUN cargo install --path skystreamer-bin

CMD ["skystreamer"]