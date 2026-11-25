Pinpoint @VERSION@ - macOS desktop client

This ZIP contains `Pinpoint.app`, an unsigned app bundle.

Install:
  1. Unzip and drag `Pinpoint.app` to Applications.
  2. On first launch, Control-click the app and choose Open (Gatekeeper
     warning for an unsigned build).

Run from Terminal:
  /Applications/Pinpoint.app/Contents/MacOS/pinpoint-desktop --backend http://127.0.0.1:8080

Use an HTTPS backend for anything other than loopback. Check the version:
  /Applications/Pinpoint.app/Contents/MacOS/pinpoint-desktop --help

Built on macos-latest (arm64). Map data © OpenStreetMap contributors (ODbL).
