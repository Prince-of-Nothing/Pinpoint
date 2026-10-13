# MVP Presentation Speech

Speech for the Canva deck "Always Together — MVP presentation" (11 slides).
The same text is in the deck's speaker notes. Slide visuals are in
[`docs/assets/presentation/`](../assets/presentation/).

## Slides 1–4 — Introduction (~235 words)

Good afternoon — I'm Andrei, and my project is Always Together: privacy-first location sharing.

Before the solution, let's look at the problem (slide 2). Location sharing is great for safety — you can see that a friend got home, or that your kid is on the way. The catch is how most apps treat friendship. Once someone is your friend, they can see where you are, all the time, until you remember to turn it off. That's the first problem: friend equals full access.

The second problem is honesty. When a phone dies or loses signal, many apps keep showing the last dot on the map as if it were live. In an emergency, an old dot that looks live is worse than no dot at all.

And the third problem is control: stopping someone from seeing you, or hiding from someone who makes you uncomfortable, is often buried or incomplete.

Our solution (slide 3) addresses this directly. Sharing is a decision, not a side effect of friendship. You choose who sees you and for how long — an hour, a day, or until you stop. You choose how precise: exact, or about one kilometre. And whoever receives it always sees how fresh it is: live, stale, or no signal.

So our goals are simple (slide 4): consent for every share, honest freshness, and security by default. Let me show you how it's built.

## Slides 5–6 — Architecture and consent (~235 words)

Now let's look at how the system actually works.

First, the architecture (slide 5). It's a modular monolith: one FastAPI backend, a PostgreSQL database and Redis, and the web client is served from the same server. The important part is the row of boxes: every request goes through the same checks, on the server, in the same order. First, security headers and a size limit. Then — who are you? We check the token and the live session behind it. Then — is the input valid? Unknown fields are rejected outright. Then — are you allowed? Only after all of that do you get a response, and it only contains what was shared with you. Underneath, the database keeps only your latest location, never a history, and the API connects with a restricted account that can't change the schema or edit the audit log.

Second, consent (slide 6) — the core idea. Alex shares his exact location with Maria for one hour. That is one arrow, in one direction. Maria gets nothing back unless she decides to share too. Being friends alone shares nothing. And every time Maria's app asks for locations, the server checks four things again: the share is active, it hasn't expired, they're still friends, and nobody blocked anybody. There is also no endpoint where you can type someone else's ID and ask for their location — so there is simply nothing to guess.

## Slides 7–8 — Sessions and security controls (~215 words)

Now — how do we keep accounts and sessions safe?

Slide 7 is the session model. When you log in, your password is checked with Argon2id, and a wrong username and a wrong password look exactly the same, even in timing. You get a token that lives only 15 minutes and is tied to a session on the server — so when you log out, it stops working immediately, not in 15 minutes. To stay logged in, the app uses a refresh token that works exactly once. If someone steals an old one and replays it, we notice the reuse and end the whole session for both sides. In the browser, that refresh token sits in a cookie JavaScript can't read, with a CSRF token on top. And five wrong passwords lock the account for 15 minutes.

Slide 8 brings it together. These are the six security controls we committed to: secure authentication, password hashing, role-based authorization, input validation, JWT and session handling, and rate limiting. And the numbers underneath are why I trust them: 140 automated tests, including attack scenarios like IDOR, injection and token forgery; 97 percent code coverage; zero findings from the bandit static analyzer; and zero known vulnerable dependencies. The test report is generated from a real run, so anyone can reproduce it.

## Slides 9–11 — Demo, roadmap and close (~190 words)

Let me show it running (slide 9 plays the recorded demo video). This is a real recording of two browsers against the running server: Maria on the left, Alex on the right. They are friends, but Maria sees nothing of Alex until he shares — exact, for one hour. Now Maria sees him live, with accuracy and battery. When Alex moves, her map follows. Jordan missed his check-in, and Alex taps "I'm OK". When Alex stops sharing, he disappears from Maria's map immediately. Then we attack our own API: reading someone else's share returns 404, making yourself admin is rejected, a replayed token is refused and kills the session, logout ends access at once, and brute force hits the rate limit.

The video (`docs/assets/presentation/always-together-demo.mp4`, 1:35) is built by `e2e/demo-video/`; talk over it and let it play. If there is time, run the live demo from the [MVP Demo Script](./mvp-demo-script.md) instead.

Finally, what's next (slide 10). In October, an Android app with real background location. In November, push notifications for check-ins and multi-factor login. In December, abuse reporting and privacy tools like account deletion and data export. After that — trails and search, but only once the safety core is solid.

Thank you for listening — I'm happy to take questions.
