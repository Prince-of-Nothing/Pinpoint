# 11 — Use Case Diagram 📋

```mermaid
flowchart LR
    User((👤 Friend User))
    Admin((🛡️ Admin/Moderator))
    Visitor((👨‍👩‍👧 Link Visitor))

    subgraph "Auth & account"
        UC_Login[Login / Logout]
        UC_ChangePW[Change forced password]
        UC_2FA[Enable 2FA — T27]
        UC_Export[Export my data — T29]
        UC_Delete[Delete my account — T29]
    end

    subgraph "Social graph"
        UC_Req[Request friend]
        UC_Accept[Accept/Reject request]
    end

    subgraph "Consent & sharing"
        UC_Grant[Create directional share<br/>precision/purpose/expiry]
        UC_Pause[Pause/resume share]
        UC_Schedule[Schedule/geofence windows — T16]
        UC_Link[Issue time-limited share link — T10]
    end

    subgraph "Location"
        UC_Publish[Publish location<br/>manual or scheduled — T21]
        UC_View[View friends' locations on map]
        UC_Realtime[Receive realtime updates — T07]
        UC_Report[Report suspicious pin — T25]
    end

    subgraph "Administration"
        UC_Invite[Invite user — T24]
        UC_Roles[Manage roles — T13]
        UC_Audit[Read audit log — T08]
        UC_Mod[Moderate reports / freeze accounts — T25]
        UC_Metrics[View system metrics — T18]
    end

    User --- UC_Login & UC_ChangePW & UC_2FA & UC_Export & UC_Delete
    User --- UC_Req & UC_Accept
    User --- UC_Grant & UC_Pause & UC_Schedule & UC_Link
    User --- UC_Publish & UC_View & UC_Realtime & UC_Report
    Admin --- UC_Invite & UC_Roles & UC_Audit & UC_Mod & UC_Metrics
    Visitor --- UC_View

    UC_View -.include.-> UC_Grant: requires active share
    UC_Realtime -.extend.-> UC_View
    UC_Grant -.include.-> UC_Accept: friendship precondition
```

Each use case maps to endpoints already implemented (Go) or tasks T03–T31 planned; requirement IDs in TRACEABILITY.md stay authoritative.
