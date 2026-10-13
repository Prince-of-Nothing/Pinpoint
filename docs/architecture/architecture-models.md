# Architecture Models

- **Maps:** OpenStreetMap with attribution and a suitable tile provider.
- **Routing:** licensed in-app routing plus external map deep links or coordinate
  export.
- **Mobile:** shared cross-platform code where practical, with native Android and
  iPhone background-location handling.
- **Backend:** accounts, relationships, consent, encrypted location data, caches,
  chats, groups, media, moderation, and expiry jobs.
- **Feature boundaries:** location, social, creative, and experimental features use
  separate permissions, data retention, and abuse controls.
- **AI:** not required for the foundation. Future recommendations or moderation
  assistance must be optional, reviewable, and never treated as proof of safety.

---
*Referenced from [§4 Solution Strategy](./solution-strategy.md) and
[§5 Building Block View](./building-block-view.md).*
