Pinpoint @VERSION@ - Linux x86_64 server and web client

This ZIP contains:

  pinpoint-server   the Axum server with the web client embedded
  README.txt        this file

Run:
  ./pinpoint-server

Then open:
  http://127.0.0.1:8080/

There is no separate web bundle: the client is embedded in the binary. Set
BIND_ADDR, PUBLIC_URL and DATABASE_URL for a real deployment. Without
DATABASE_URL the server runs in memory and does not persist data.

Health check:
  http://127.0.0.1:8080/health   -> {"version":"@VERSION@", ...}

This build is unsigned. Map data © OpenStreetMap contributors (ODbL).
