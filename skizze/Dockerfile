FROM python:3.12-alpine
WORKDIR /app
COPY server.py karte.js index.html icon.svg ./
ENV DATEN=/daten PORT=8095 PYTHONUNBUFFERED=1
VOLUME /daten
EXPOSE 8095
HEALTHCHECK --interval=60s --timeout=5s CMD wget -qO- http://127.0.0.1:8095/gesund >/dev/null || exit 1
CMD ["python3", "/app/server.py"]
