# ---- Build stage -----------------------------------------------------------
FROM golang:1.22-alpine AS build

WORKDIR /src

# Cache module downloads separately from source changes.
COPY go.mod go.sum ./
RUN go mod download

COPY . .

# Static binary, no CGO; version stamped via ldflags.
ARG VERSION=dev
RUN CGO_ENABLED=0 GOOS=linux go build -trimpath \
        -ldflags "-s -w -X main.Version=${VERSION}" \
        -o /out/server ./cmd/server

# ---- Runtime stage ---------------------------------------------------------
FROM alpine:3.20

RUN apk add --no-cache ca-certificates curl && \
    addgroup -S app && adduser -S -G app app

WORKDIR /app
COPY --from=build /out/server /app/server

USER app
EXPOSE 8000

HEALTHCHECK --interval=30s --timeout=5s --start-period=5s --retries=3 \
    CMD curl -fsS http://127.0.0.1:8000/health || exit 1

ENTRYPOINT ["/app/server"]
