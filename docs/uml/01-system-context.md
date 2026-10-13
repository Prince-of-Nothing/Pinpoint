# 01 — System Context (C4 L1) 📋

```mermaid
graph TB
    subgraph Actors
        U[👤 End User<br/>shares & views locations]
        A[🛡️ Administrator<br/>provisions users, moderates]
        K[👨‍👩‍👧 Family/Kiosk Visitor<br/>views via share link, no account]
    end

    SYS((GoLocate System<br/>consent-based location sharing))

    OSM[OSM Tile Servers<br/>external: map imagery]
    FCM[Firebase Cloud Messaging<br/>external: push delivery — T17]
    SMTP[SMTP Provider<br/>external: invites/alerts — T24]
    GEO[Device GPS / Network Location<br/>external: position source]

    U -->|uses| WEB[web SPA]
    U -->|uses| AND[Android app Kotlin — T04..T31]
    A -->|uses| WEBA[weba Admin console — T03/T13]
    K -->|opens QR link| WEBK[webk Kiosk view — T10]
    WEB --> SYS
    AND --> SYS
    WEBA --> SYS
    WEBK --> SYS
    SYS -->|fetch tiles via clients| OSM
    SYS -->|push notifications| FCM
    SYS -->|emails| SMTP
    U -->|location readings| GEO
```

**Notes**
- The four clients all talk to the same Go API (`/api/v1`); `webk` uses short-lived signed share tokens instead of accounts.
- Tiles are fetched by the browsers/app directly from OSM (system stores only coordinates).
- Traceability: FR-consent, FR-friendship, FR-map (see TRACEABILITY.md), tasks T03/T04/T10/T17/T24.
