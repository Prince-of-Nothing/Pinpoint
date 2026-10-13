# 08 — State Diagrams: Share & Account 📋

## Share lifecycle (directional consent)

```mermaid
stateDiagram-v2
    [*] --> Active : owner creates share to accepted friend
    Active --> Paused : owner pauses (manual)
    Paused --> Active : owner resumes
    Active --> Expired : ExpiresAt reached (sweeper job T06)
    Paused --> Expired : ExpiresAt reached
    Active --> Revoked : owner deletes share
    Paused --> Revoked : owner deletes share
    Active --> Blocked : geofence/schedule window closed (T16)
    Blocked --> Active : window opens again (T16)
    Expired --> [*]
    Revoked --> [*]

    note right of Active
        Viewer reads locations at the
        granted precision only.
        Every read is audit-logged (T08).
    end note
```

## Account lifecycle

```mermaid
stateDiagram-v2
    [*] --> Invited : admin invite (T24 email)
    Invited --> MustChangePW : first login with temp password
    MustChangePW --> Active : password set (policy-checked)
    Active --> Locked : failed-login threshold exceeded
    Locked --> Active : auto-unlock after cooldown OR admin unlock
    Active --> Suspended : moderator action (T25)
    Suspended --> Active : moderator reinstates
    Active --> ErasureQueued : GDPR deletion requested (T29)
    Suspended --> ErasureQueued : GDPR deletion requested
    ErasureQueued --> Deleted : cascade purge (locs, shares, audit anonymized) 
    Deleted --> [*]
```

Both machines are enforced server-side in handler guards; clients merely reflect them (badges in web/weba, screens in Android).
