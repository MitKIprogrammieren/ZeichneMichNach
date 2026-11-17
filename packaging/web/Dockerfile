# ZeichneMichNach im Browser – Container für Server/NAS/NUC.
# Bauen (im Repo-Hauptordner):  docker build -f packaging/web/Dockerfile -t zeichnemichnach-web .
FROM rust:1-bookworm AS bau
WORKDIR /src
COPY core core
COPY app/src app/src
COPY app/assets app/assets
COPY packaging/icon.png packaging/icon.png
COPY tools/zmn-web tools/zmn-web
COPY tools/zmn-pruefstand tools/zmn-pruefstand
RUN cd tools/zmn-web && cargo build --release \
 && cd ../zmn-pruefstand && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends \
      tesseract-ocr tesseract-ocr-deu tesseract-ocr-eng \
      fonts-dejavu-core fonts-liberation2 fonts-freefont-ttf fonts-lato fonts-open-sans \
      fonts-roboto-unhinted fonts-cantarell ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=bau /src/tools/zmn-web/target/release/zmn-web /app/zmn-web
COPY --from=bau /src/tools/zmn-pruefstand/target/release/zmn-pruefstand /app/zmn-pruefstand
COPY packaging/linux/zmn-ocr /app/zmn-ocr
RUN chmod +x /app/zmn-ocr && mkdir -p /daten/schriften
# eigene Schriften (z. B. die vom Mac) einfach nach /daten/schriften legen
ENV ZMN_DATEN=/daten ZMN_PORT=8094 ZMN_FONTS=/usr/share/fonts:/daten/schriften TZ=Europe/Berlin
EXPOSE 8094
VOLUME /daten
HEALTHCHECK --interval=30s --timeout=5s CMD ["/bin/bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8094 || exit 1"]
ENTRYPOINT ["/app/zmn-web"]
