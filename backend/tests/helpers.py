"""Test helpers for creating accounts and authenticated requests."""

from dataclasses import dataclass

DEFAULT_PASSWORD = "correct horse battery staple"


@dataclass
class Account:
    id: str
    username: str
    password: str
    access_token: str
    refresh_token: str | None

    @property
    def headers(self) -> dict[str, str]:
        return {"Authorization": f"Bearer {self.access_token}"}


def register(client, username: str, password: str = DEFAULT_PASSWORD, **extra):
    return client.post(
        "/api/v1/auth/register",
        json={
            "username": username,
            "email": f"{username}@example.com",
            "password": password,
            **extra,
        },
    )


def login(client, login: str, password: str = DEFAULT_PASSWORD, delivery: str = "body"):
    return client.post(
        "/api/v1/auth/login",
        json={"login": login, "password": password, "token_delivery": delivery},
    )


def create_account(client, username: str, password: str = DEFAULT_PASSWORD) -> Account:
    response = register(client, username, password)
    assert response.status_code == 201, response.text
    tokens = login(client, username, password)
    assert tokens.status_code == 200, tokens.text
    body = tokens.json()
    return Account(
        id=body["user"]["id"],
        username=username,
        password=password,
        access_token=body["access_token"],
        refresh_token=body["refresh_token"],
    )


def befriend(client, a: Account, b: Account) -> str:
    request = client.post("/api/v1/friendships", headers=a.headers, json={"username": b.username})
    assert request.status_code == 201, request.text
    friendship_id = request.json()["id"]
    accept = client.post(f"/api/v1/friendships/{friendship_id}/accept", headers=b.headers)
    assert accept.status_code == 200, accept.text
    return friendship_id


def share(client, owner: Account, recipient: Account, **options) -> dict:
    response = client.post(
        "/api/v1/location-shares",
        headers=owner.headers,
        json={"recipient_username": recipient.username, **options},
    )
    assert response.status_code == 201, response.text
    return response.json()
