// End-to-end encryption for 1:1 messages using WebCrypto only.
//
// This mirrors `src/security/message_crypto.rs`: X25519 + HKDF-SHA256 +
// AES-256-GCM with an Ed25519 signature. It is an ECIES-style construction,
// not the Signal Protocol; see ADR-0023 for the limits.
//
// Private keys are generated extractable only long enough to export the
// public half, then re-imported non-extractable and stored as CryptoKey
// objects in IndexedDB. JavaScript can use them but cannot export the bytes.

const DB_NAME = "pinpoint-messages";
const DB_STORE = "identities";
const TO_INFO = new TextEncoder().encode("pinpoint-message-v1-to");
const FROM_INFO = new TextEncoder().encode("pinpoint-message-v1-from");
const MAX_CHARS = 2000;

const cache = new Map();

export class MessageCryptoUnavailable extends Error {}

export function cryptoAvailable() {
  return Boolean(globalThis.crypto?.subtle) && typeof indexedDB !== "undefined";
}

function b64(bytes) {
  let binary = "";
  for (const byte of new Uint8Array(bytes)) binary += String.fromCharCode(byte);
  return btoa(binary);
}

function unb64(value) {
  const binary = atob(value);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}

function concat(...parts) {
  const length = parts.reduce((total, part) => total + part.length, 0);
  const out = new Uint8Array(length);
  let offset = 0;
  for (const part of parts) {
    out.set(part, offset);
    offset += part.length;
  }
  return out;
}

function i64le(value) {
  const buffer = new ArrayBuffer(8);
  new DataView(buffer).setBigInt64(0, BigInt(value), true);
  return new Uint8Array(buffer);
}

function u32le(value) {
  const buffer = new ArrayBuffer(4);
  new DataView(buffer).setUint32(0, value, true);
  return new Uint8Array(buffer);
}

function randomBytes(length) {
  const out = new Uint8Array(length);
  crypto.getRandomValues(out);
  return out;
}

function signingPayload(senderId, recipientId, fields) {
  const out = [new Uint8Array([1]), i64le(senderId), i64le(recipientId)];
  for (const field of fields) out.push(u32le(field.length), field);
  return concat(...out);
}

async function deriveMessageKey(shared, salt, info) {
  const material = await crypto.subtle.importKey("raw", shared, "HKDF", false, ["deriveBits"]);
  return new Uint8Array(
    await crypto.subtle.deriveBits(
      { name: "HKDF", hash: "SHA-256", salt, info },
      material,
      256,
    ),
  );
}

async function encrypt(key, nonce, text) {
  const material = await crypto.subtle.importKey("raw", key, { name: "AES-GCM" }, false, ["encrypt"]);
  return new Uint8Array(
    await crypto.subtle.encrypt(
      { name: "AES-GCM", iv: nonce },
      material,
      new TextEncoder().encode(text),
    ),
  );
}

async function decrypt(key, nonce, ciphertext) {
  const material = await crypto.subtle.importKey("raw", key, { name: "AES-GCM" }, false, ["decrypt"]);
  return new Uint8Array(
    await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce }, material, ciphertext),
  );
}

async function sharedSecret(privateKey, publicBytes) {
  const publicKey = await crypto.subtle.importKey(
    "raw",
    publicBytes,
    { name: "X25519" },
    false,
    [],
  );
  return new Uint8Array(
    await crypto.subtle.deriveBits({ name: "X25519", public: publicKey }, privateKey, 256),
  );
}

async function generateIdentity() {
  if (!cryptoAvailable()) {
    throw new MessageCryptoUnavailable("This browser does not support X25519/Ed25519 WebCrypto.");
  }
  const x = await crypto.subtle.generateKey({ name: "X25519" }, true, ["deriveBits"]);
  const e = await crypto.subtle.generateKey({ name: "Ed25519" }, true, ["sign", "verify"]);
  const xPublic = new Uint8Array(await crypto.subtle.exportKey("raw", x.publicKey));
  const ePublic = new Uint8Array(await crypto.subtle.exportKey("raw", e.publicKey));
  // Re-import private keys non-extractable; the stored CryptoKeys cannot be
  // exported back to raw bytes by page script.
  const xPrivatePkcs8 = await crypto.subtle.exportKey("pkcs8", x.privateKey);
  const ePrivatePkcs8 = await crypto.subtle.exportKey("pkcs8", e.privateKey);
  const xPrivate = await crypto.subtle.importKey(
    "pkcs8",
    xPrivatePkcs8,
    { name: "X25519" },
    false,
    ["deriveBits"],
  );
  const ePrivate = await crypto.subtle.importKey(
    "pkcs8",
    ePrivatePkcs8,
    { name: "Ed25519" },
    false,
    ["sign"],
  );
  return {
    x25519Private: xPrivate,
    ed25519Private: ePrivate,
    publicBundle: { x25519: b64(xPublic), ed25519: b64(ePublic) },
  };
}

function openDb() {
  return new Promise((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => {
      const db = request.result;
      if (!db.objectStoreNames.contains(DB_STORE)) db.createObjectStore(DB_STORE);
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error || new Error("IndexedDB unavailable"));
  });
}

async function loadStored(userId) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const request = db.transaction(DB_STORE, "readonly").objectStore(DB_STORE).get(String(userId));
    request.onsuccess = () => resolve(request.result || null);
    request.onerror = () => reject(request.error || new Error("Could not read message key"));
  });
}

async function saveStored(userId, identity) {
  const db = await openDb();
  return new Promise((resolve, reject) => {
    const request = db
      .transaction(DB_STORE, "readwrite")
      .objectStore(DB_STORE)
      .put(identity, String(userId));
    request.onsuccess = () => resolve();
    request.onerror = () => reject(request.error || new Error("Could not store message key"));
  });
}

/** Load or create the per-account identity. Never returns secret bytes. */
export async function ensureIdentity(userId) {
  const key = String(userId);
  if (cache.has(key)) return cache.get(key);
  let identity = await loadStored(key);
  if (!identity) {
    identity = await generateIdentity();
    await saveStored(key, identity);
  }
  cache.set(key, identity);
  return identity;
}

export function publicBundle(identity) {
  return identity.publicBundle;
}

/** Seal a message for a friend and keep a copy for the sender. */
export async function seal(identity, recipient, senderId, recipientId, text) {
  const trimmed = String(text ?? "").trim();
  if (
    !recipient?.x25519 ||
    !recipient?.ed25519 ||
    !trimmed ||
    [...trimmed].length > MAX_CHARS ||
    senderId === recipientId
  ) {
    return null;
  }
  const ephemeral = await crypto.subtle.generateKey({ name: "X25519" }, true, ["deriveBits"]);
  const ephemeralPublic = new Uint8Array(
    await crypto.subtle.exportKey("raw", ephemeral.publicKey),
  );
  const toShared = await sharedSecret(ephemeral.privateKey, unb64(recipient.x25519));
  const fromShared = await sharedSecret(
    ephemeral.privateKey,
    unb64(identity.publicBundle.x25519),
  );

  const saltTo = randomBytes(16);
  const saltFrom = randomBytes(16);
  const nonceTo = randomBytes(12);
  const nonceFrom = randomBytes(12);
  const keyTo = await deriveMessageKey(toShared, saltTo, TO_INFO);
  const keyFrom = await deriveMessageKey(fromShared, saltFrom, FROM_INFO);
  const ciphertextTo = await encrypt(keyTo, nonceTo, trimmed);
  const ciphertextFrom = await encrypt(keyFrom, nonceFrom, trimmed);
  const payload = signingPayload(senderId, recipientId, [
    ephemeralPublic,
    saltTo,
    nonceTo,
    ciphertextTo,
    saltFrom,
    nonceFrom,
    ciphertextFrom,
  ]);
  const signature = new Uint8Array(
    await crypto.subtle.sign({ name: "Ed25519" }, identity.ed25519Private, payload),
  );
  return {
    ephemeral_public_key: b64(ephemeralPublic),
    salt_to: b64(saltTo),
    nonce_to: b64(nonceTo),
    ciphertext_to: b64(ciphertextTo),
    salt_from: b64(saltFrom),
    nonce_from: b64(nonceFrom),
    ciphertext_from: b64(ciphertextFrom),
    signature: b64(signature),
  };
}

/** Verify and decrypt one envelope. Returns null on any failure. */
export async function open(identity, sender, senderId, recipientId, sealed, asRecipient) {
  try {
    if (!sender?.ed25519 || !sealed?.signature) return null;
    const verifyKey = await crypto.subtle.importKey(
      "raw",
      unb64(sender.ed25519),
      { name: "Ed25519" },
      false,
      ["verify"],
    );
    const payload = signingPayload(senderId, recipientId, [
      unb64(sealed.ephemeral_public_key),
      unb64(sealed.salt_to),
      unb64(sealed.nonce_to),
      unb64(sealed.ciphertext_to),
      unb64(sealed.salt_from),
      unb64(sealed.nonce_from),
      unb64(sealed.ciphertext_from),
    ]);
    const valid = await crypto.subtle.verify(
      { name: "Ed25519" },
      verifyKey,
      unb64(sealed.signature),
      payload,
    );
    if (!valid) return null;
    const shared = await sharedSecret(
      identity.x25519Private,
      unb64(sealed.ephemeral_public_key),
    );
    const salt = unb64(asRecipient ? sealed.salt_to : sealed.salt_from);
    const nonce = unb64(asRecipient ? sealed.nonce_to : sealed.nonce_from);
    const ciphertext = unb64(
      asRecipient ? sealed.ciphertext_to : sealed.ciphertext_from,
    );
    const key = await deriveMessageKey(
      shared,
      salt,
      asRecipient ? TO_INFO : FROM_INFO,
    );
    const text = new TextDecoder().decode(await decrypt(key, nonce, ciphertext));
    return text && [...text].length <= MAX_CHARS ? text : null;
  } catch (_) {
    return null;
  }
}

/** Safety fingerprint: SHA-256(x25519 || ed25519), first 16 bytes. */
export async function fingerprint(bundle) {
  try {
    const digest = await crypto.subtle.digest(
      "SHA-256",
      concat(unb64(bundle.x25519), unb64(bundle.ed25519)),
    );
    return [...new Uint8Array(digest)]
      .slice(0, 16)
      .map((byte) => byte.toString(16).padStart(2, "0"))
      .join(":");
  } catch (_) {
    return null;
  }
}
