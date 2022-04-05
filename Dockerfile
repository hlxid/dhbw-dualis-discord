FROM rust AS build

WORKDIR /app
COPY . .
RUN cargo build --release


FROM debian

LABEL org.opencontainers.image.source = "https://github.com/daniel0611/DHBW-Dualis"
WORKDIR /data

RUN apt update && apt install ca-certificates -y && rm -rf /var/lib/apt/lists/*

RUN mkdir /app
COPY --from=build /app/target/release/dhbw_dualis /app

CMD [ "/app/dhbw_dualis" ]