Pinpoint @VERSION@ - server and web client

Run:
  pinpoint-server.exe

Then open:
  http://127.0.0.1:8080/

The web client is embedded in this binary; there is no separate web bundle.
Set BIND_ADDR, PUBLIC_URL and DATABASE_URL for a real deployment. Without
DATABASE_URL the server runs in memory and does not persist data.

Health check:
  http://127.0.0.1:8080/health   -> {"version":"@VERSION@", ...}

This build is unsigned.
