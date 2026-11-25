Pinpoint @VERSION@ - Linux x86_64 desktop client

The ZIP contains:
  pinpoint-desktop   the client
  pinpoint.desktop   desktop entry
  pinpoint.png       icon
  README.txt         this file

Run without installing:
  ./pinpoint-desktop --backend http://127.0.0.1:8080

Install for the current user:
  install -Dm755 pinpoint-desktop ~/.local/bin/pinpoint-desktop
  install -Dm644 pinpoint.desktop ~/.local/share/applications/pinpoint.desktop
  install -Dm644 pinpoint.png ~/.local/share/icons/hicolor/192x192/apps/pinpoint.png

Use an HTTPS backend for anything other than loopback. Check the version:
  ./pinpoint-desktop --help

This build is unsigned. Map data © OpenStreetMap contributors (ODbL).
