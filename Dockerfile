FROM python:3.11-slim

WORKDIR /app

RUN pip install --no-cache-dir flask requests

COPY proxy.py config.py launch-codex.sh ./

RUN chmod +x launch-codex.sh

ENV CODEX_PROXY_API_KEY=""
ENV UPSTREAM_URL=""
ENV PORT=9090
ENV HOST=0.0.0.0

EXPOSE 9090

CMD ["sh", "-c", "python proxy.py --upstream ${UPSTREAM_URL} --api-key ${CODEX_PROXY_API_KEY} --port ${PORT} --host ${HOST}"]