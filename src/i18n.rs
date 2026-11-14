//! Internationalization (i18n) — every language the app can show.
//!
//! Design: a `Lang` registry (ISO 639-1) plus a translation table keyed by
//! `(Lang, key)`. Missing translations fall back to English, so a new
//! language is *additive*: add a `Lang` variant, add its table, done. The
//! language is auto-detected from the OS locale (env `LANG`/`LC_ALL`/
//! `LC_MESSAGES`) and can be overridden in Settings.
//!
//! Keys are stable identifiers; `t(lang, key)` returns the string. No
//! runtime string allocation for the fallback path.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Supported UI languages. Keep the codes ISO 639-1 (and Moldovan = `ro-MD`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[allow(non_camel_case_types)]
pub enum Lang {
    En,
    Ro,
    Ru,
    Fr,
    De,
    Es,
    Pt,
    It,
    Nl,
    Pl,
    Uk,
    Be,
    Tr,
    Ar,
    He,
    Fa,
    Hi,
    Bn,
    Zh,
    Ja,
    Ko,
    Vi,
    Th,
    Id,
    El,
    Cs,
    Sk,
    Hu,
    Bg,
    Hr,
    Sr,
    Lt,
    Lv,
    Et,
    Fi,
    Sv,
    No,
    Da,
    Is,
    Ca,
    Eu,
    Gl,
    Af,
    Sw,
    Am,
    Yo,
    Ig,
    Zu,
    Sq,
    Mk,
    Bs,
    Ka,
    Hy,
    Az,
    Kk,
    Uz,
    Mn,
    Ne,
    Si,
    Ta,
    Te,
    Ml,
    Kn,
    Gu,
    Mr,
    Pa,
    Ur,
    Ps,
    So,
    Ha,
    Rw,
    Ny,
    Mg,
    St,
    Tn,
    Xh,
    Sn,
    Nn,
}

impl Lang {
    /// All languages, in display order. English first (fallback).
    pub fn all() -> &'static [Lang] {
        use Lang::*;
        &[
            En, Ro, Ru, Fr, De, Es, Pt, It, Nl, Pl, Uk, Be, Tr, Ar, He, Fa, Hi, Bn, Zh, Ja, Ko, Vi,
            Th, Id, El, Cs, Sk, Hu, Bg, Hr, Sr, Lt, Lv, Et, Fi, Sv, No, Da, Is, Ca, Eu, Gl, Af, Sw,
            Am, Yo, Ig, Zu, Sq, Mk, Bs, Ka, Hy, Az, Kk, Uz, Mn, Ne, Si, Ta, Te, Ml, Kn, Gu, Mr, Pa,
            Ur, Ps, So, Ha, Rw, Ny, Mg, St, Tn, Xh, Sn, Nn,
        ]
    }

    /// BCP-47-ish code for Nominatim `accept-language` / locale files.
    pub fn code(self) -> &'static str {
        use Lang::*;
        match self {
            En => "en",
            Ro => "ro",
            Ru => "ru",
            Fr => "fr",
            De => "de",
            Es => "es",
            Pt => "pt",
            It => "it",
            Nl => "nl",
            Pl => "pl",
            Uk => "uk",
            Be => "be",
            Tr => "tr",
            Ar => "ar",
            He => "he",
            Fa => "fa",
            Hi => "hi",
            Bn => "bn",
            Zh => "zh",
            Ja => "ja",
            Ko => "ko",
            Vi => "vi",
            Th => "th",
            Id => "id",
            El => "el",
            Cs => "cs",
            Sk => "sk",
            Hu => "hu",
            Bg => "bg",
            Hr => "hr",
            Sr => "sr",
            Lt => "lt",
            Lv => "lv",
            Et => "et",
            Fi => "fi",
            Sv => "sv",
            No => "no",
            Da => "da",
            Is => "is",
            Ca => "ca",
            Eu => "eu",
            Gl => "gl",
            Af => "af",
            Sw => "sw",
            Am => "am",
            Yo => "yo",
            Ig => "ig",
            Zu => "zu",
            Sq => "sq",
            Mk => "mk",
            Bs => "bs",
            Ka => "ka",
            Hy => "hy",
            Az => "az",
            Kk => "kk",
            Uz => "uz",
            Mn => "mn",
            Ne => "ne",
            Si => "si",
            Ta => "ta",
            Te => "te",
            Ml => "ml",
            Kn => "kn",
            Gu => "gu",
            Mr => "mr",
            Pa => "pa",
            Ur => "ur",
            Ps => "ps",
            So => "so",
            Ha => "ha",
            Rw => "rw",
            Ny => "ny",
            Mg => "mg",
            St => "st",
            Tn => "tn",
            Xh => "xh",
            Sn => "sn",
            Nn => "nn",
        }
    }

    /// Native endonym for the language picker.
    pub fn native_name(self) -> &'static str {
        use Lang::*;
        match self {
            En => "English",
            Ro => "Română",
            Ru => "Русский",
            Fr => "Français",
            De => "Deutsch",
            Es => "Español",
            Pt => "Português",
            It => "Italiano",
            Nl => "Nederlands",
            Pl => "Polski",
            Uk => "Українська",
            Be => "Беларуская",
            Tr => "Türkçe",
            Ar => "العربية",
            He => "עברית",
            Fa => "فارسی",
            Hi => "हिन्दी",
            Bn => "বাংলা",
            Zh => "中文",
            Ja => "日本語",
            Ko => "한국어",
            Vi => "Tiếng Việt",
            Th => "ไทย",
            Id => "Bahasa Indonesia",
            El => "Ελληνικά",
            Cs => "Čeština",
            Sk => "Slovenčina",
            Hu => "Magyar",
            Bg => "Български",
            Hr => "Hrvatski",
            Sr => "Српски",
            Lt => "Lietuvių",
            Lv => "Latviešu",
            Et => "Eesti",
            Fi => "Suomi",
            Sv => "Svenska",
            No => "Norsk",
            Da => "Dansk",
            Is => "Íslenska",
            Ca => "Català",
            Eu => "Euskara",
            Gl => "Galego",
            Af => "Afrikaans",
            Sw => "Kiswahili",
            Am => "አማርኛ",
            Yo => "Yorùbá",
            Ig => "Igbo",
            Zu => "isiZulu",
            Sq => "Shqip",
            Mk => "Македонски",
            Bs => "Bosanski",
            Ka => "ქართული",
            Hy => "Հայերեն",
            Az => "Azərbaycan",
            Kk => "Қазақ",
            Uz => "Oʻzbek",
            Mn => "Монгол",
            Ne => "नेपाली",
            Si => "සිංහල",
            Ta => "தமிழ்",
            Te => "తెలుగు",
            Ml => "മലയാളം",
            Kn => "ಕನ್ನಡ",
            Gu => "ગુજરાતી",
            Mr => "मराठी",
            Pa => "ਪੰਜਾਬੀ",
            Ur => "اردو",
            Ps => "پښتو",
            So => "Soomaali",
            Ha => "Hausa",
            Rw => "Kinyarwanda",
            Ny => "Chichewa",
            Mg => "Malagasy",
            St => "Sesotho",
            Tn => "Setswana",
            Xh => "isiXhosa",
            Sn => "ChiShona",
            Nn => "Nederlands (Nederland)",
        }
    }

    /// Parse a locale string (`ro-MD`, `ro_MD`, `ro`, `ru-RU`) into a Lang.
    pub fn from_locale(locale: &str) -> Option<Self> {
        let base = locale.split(['-', '_']).next().unwrap_or("").to_lowercase();
        Lang::all().iter().copied().find(|l| l.code() == base)
    }

    /// Detect the UI language from the OS locale (env vars), fallback English.
    pub fn system() -> Self {
        for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
            if let Ok(v) = std::env::var(key) {
                if let Some(l) = Self::from_locale(&v) {
                    return l;
                }
            }
        }
        Self::En
    }
}

/// Stable translation keys (add new keys here; English is the fallback).
pub mod key {
    pub const APP_TITLE: &str = "app_title";
    pub const MAP: &str = "map";
    pub const FRIENDS: &str = "friends";
    pub const SHARING: &str = "sharing";
    pub const PLACES: &str = "places";
    pub const SAVED: &str = "saved";
    pub const TRIPS: &str = "trips";
    pub const EMERGENCY: &str = "emergency";
    pub const PROFILE: &str = "profile";
    pub const ADMIN: &str = "admin";
    pub const KIOSK: &str = "kiosk";
    pub const PASSWORD: &str = "password";
    pub const SIGN_IN: &str = "sign_in";
    pub const SIGN_OUT: &str = "sign_out";
    pub const OFFLINE_MODE: &str = "offline_mode";
    pub const REFRESH: &str = "refresh";
    pub const FRIEND_LOCATIONS: &str = "friend_locations";
    pub const PUBLISH_MY_LOCATION: &str = "publish_my_location";
    pub const LATITUDE: &str = "latitude";
    pub const LONGITUDE: &str = "longitude";
    pub const PUBLISH: &str = "publish";
    pub const NO_FRIENDS: &str = "no_friends";
    pub const ACCEPT: &str = "accept";
    pub const REJECT: &str = "reject";
    pub const AWAITING_RESPONSE: &str = "awaiting_response";
    pub const LOCATION_SHARING: &str = "location_sharing";
    pub const NO_SHARING: &str = "no_sharing";
    pub const PAUSE: &str = "pause";
    pub const RESUME: &str = "resume";
    pub const NAVIGATE_TO_FRIEND: &str = "navigate_to_friend";
    pub const NAVIGATE: &str = "navigate";
    pub const OSM_MAP: &str = "osm_map";
    pub const NO_LOCATIONS: &str = "no_locations";
    pub const SEARCH_PLACES: &str = "search_places";
    pub const SEARCH: &str = "search";
    pub const RESULTS: &str = "results";
    pub const LANGUAGE: &str = "language";
    pub const PROFILE_VISIBILITY: &str = "profile_visibility";
    pub const FRIENDS_ONLY: &str = "friends_only";
    pub const PUBLIC: &str = "public";
    pub const SIGN_IN_REQUIRED: &str = "sign_in_required";
    pub const SIGN_IN_TO_USE: &str = "sign_in_to_use";
    pub const GO_TO_MAP: &str = "go_to_map";
    pub const OFFLINE_MAP_HINT: &str = "offline_map_hint";
}

/// English fallback table (the canonical key list).
fn en() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(APP_TITLE, "Pinpoint");
    m.insert(MAP, "Map");
    m.insert(FRIENDS, "Friends");
    m.insert(SHARING, "Sharing");
    m.insert(PLACES, "Places");
    m.insert(SAVED, "Saved");
    m.insert(TRIPS, "Trips");
    m.insert(EMERGENCY, "Emergency");
    m.insert(PROFILE, "Profile");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Password");
    m.insert(SIGN_IN, "Sign in");
    m.insert(SIGN_OUT, "Sign out");
    m.insert(OFFLINE_MODE, "Offline mode");
    m.insert(REFRESH, "Refresh");
    m.insert(FRIEND_LOCATIONS, "Friend locations");
    m.insert(PUBLISH_MY_LOCATION, "Publish my location");
    m.insert(LATITUDE, "Latitude");
    m.insert(LONGITUDE, "Longitude");
    m.insert(PUBLISH, "Publish");
    m.insert(NO_FRIENDS, "No friends yet");
    m.insert(ACCEPT, "Accept");
    m.insert(REJECT, "Reject");
    m.insert(AWAITING_RESPONSE, "awaiting response");
    m.insert(LOCATION_SHARING, "Location Sharing");
    m.insert(NO_SHARING, "No location sharing permissions");
    m.insert(PAUSE, "Pause");
    m.insert(RESUME, "Resume");
    m.insert(NAVIGATE_TO_FRIEND, "Navigate to a friend");
    m.insert(NAVIGATE, "Navigate");
    m.insert(OSM_MAP, "OSM Map");
    m.insert(NO_LOCATIONS, "No locations to navigate to.");
    m.insert(SEARCH_PLACES, "Search places (OpenStreetMap)");
    m.insert(SEARCH, "Search");
    m.insert(RESULTS, "Results");
    m.insert(LANGUAGE, "Language");
    m.insert(PROFILE_VISIBILITY, "Profile visibility");
    m.insert(FRIENDS_ONLY, "Friends only");
    m.insert(PUBLIC, "Public");
    m.insert(SIGN_IN_REQUIRED, "Sign in required");
    m.insert(
        SIGN_IN_TO_USE,
        "To use {feature} features, please sign in first.",
    );
    m.insert(GO_TO_MAP, "Go to Map");
    m.insert(
        OFFLINE_MAP_HINT,
        "Or stay in Map mode — the map works offline without signing in.",
    );
    m
}

/// Translation table for every language. Languages without a block fall
/// back to English, so the registry can list any ISO 639-1 code.
fn table() -> &'static HashMap<Lang, HashMap<&'static str, &'static str>> {
    static TABLE: OnceLock<HashMap<Lang, HashMap<&'static str, &'static str>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = HashMap::new();
        t.insert(Lang::En, en());
        t.insert(Lang::Ro, ro());
        t.insert(Lang::Ru, ru());
        t.insert(Lang::Fr, fr());
        t.insert(Lang::De, de());
        t.insert(Lang::Es, es());
        t.insert(Lang::Pt, pt());
        t.insert(Lang::It, it());
        t.insert(Lang::Nl, nl());
        t.insert(Lang::Pl, pl());
        t.insert(Lang::Uk, uk());
        t.insert(Lang::Tr, tr_tr());
        t.insert(Lang::Ar, ar());
        t.insert(Lang::He, he());
        t.insert(Lang::Zh, zh());
        t.insert(Lang::Ja, ja());
        t.insert(Lang::Ko, ko());
        t.insert(Lang::Hi, hi());
        t.insert(Lang::Vi, vi());
        t.insert(Lang::Th, th());
        t.insert(Lang::Id, id());
        t.insert(Lang::El, el());
        t.insert(Lang::Cs, cs());
        t.insert(Lang::Hu, hu());
        t.insert(Lang::Sv, sv());
        t.insert(Lang::No, no());
        t.insert(Lang::Da, da());
        t.insert(Lang::Fi, fi());
        // Brand name is untranslated: ensure every block has APP_TITLE.
        for m in t.values_mut() {
            m.entry(key::APP_TITLE).or_insert("Pinpoint");
        }
        t
    })
}

/// Translate `key` into `lang`, falling back to English.
pub fn t(lang: Lang, key: &str) -> String {
    if key == key::APP_TITLE {
        return "Pinpoint".to_string();
    }
    let table = table();
    if let Some(m) = table.get(&lang) {
        if let Some(s) = m.get(key) {
            return s.to_string();
        }
    }
    table
        .get(&Lang::En)
        .and_then(|m| m.get(key))
        .map(|s| s.to_string())
        .unwrap_or_else(|| key.to_string())
}

// --- Translation blocks (add one per language; missing keys use English) ---

fn ro() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(APP_TITLE, "Pinpoint");
    m.insert(MAP, "Harta");
    m.insert(FRIENDS, "Prieteni");
    m.insert(SHARING, "Partajare");
    m.insert(PLACES, "Locuri");
    m.insert(SAVED, "Salvate");
    m.insert(TRIPS, "Călătorii");
    m.insert(EMERGENCY, "Urgență");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Parolă");
    m.insert(SIGN_IN, "Autentificare");
    m.insert(SIGN_OUT, "Deconectare");
    m.insert(OFFLINE_MODE, "Mod offline");
    m.insert(REFRESH, "Reîmprospătare");
    m.insert(FRIEND_LOCATIONS, "Locațiile prietenilor");
    m.insert(PUBLISH_MY_LOCATION, "Publică locația mea");
    m.insert(LATITUDE, "Latitudine");
    m.insert(LONGITUDE, "Longitudine");
    m.insert(PUBLISH, "Publică");
    m.insert(NO_FRIENDS, "Niciun prieten încă");
    m.insert(ACCEPT, "Acceptă");
    m.insert(REJECT, "Respinge");
    m.insert(AWAITING_RESPONSE, "se așteaptă răspuns");
    m.insert(LOCATION_SHARING, "Partajarea locației");
    m.insert(NO_SHARING, "Nicio permisiune de partajare");
    m.insert(PAUSE, "Pauză");
    m.insert(RESUME, "Reia");
    m.insert(NAVIGATE_TO_FRIEND, "Navighează la un prieten");
    m.insert(NAVIGATE, "Navighează");
    m.insert(OSM_MAP, "Harta OSM");
    m.insert(NO_LOCATIONS, "Nicio locație de navigat.");
    m.insert(SEARCH_PLACES, "Caută locuri (OpenStreetMap)");
    m.insert(SEARCH, "Caută");
    m.insert(RESULTS, "Rezultate");
    m.insert(LANGUAGE, "Limba");
    m.insert(PROFILE_VISIBILITY, "Vizibilitate profil");
    m.insert(FRIENDS_ONLY, "Doar prietenii");
    m.insert(PUBLIC, "Public");
    m.insert(SIGN_IN_REQUIRED, "Autentificare obligatorie");
    m.insert(
        SIGN_IN_TO_USE,
        "Pentru funcțiile {feature}, autentifică-te mai întâi.",
    );
    m.insert(GO_TO_MAP, "Mergi la Hartă");
    m.insert(
        OFFLINE_MAP_HINT,
        "Sau rămâi în modul Hartă — funcționează offline fără autentificare.",
    );
    m
}

fn ru() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Карта");
    m.insert(FRIENDS, "Друзья");
    m.insert(SHARING, "Передача");
    m.insert(PLACES, "Места");
    m.insert(SAVED, "Сохранённые");
    m.insert(TRIPS, "Поездки");
    m.insert(EMERGENCY, "Экстренная помощь");
    m.insert(PROFILE, "Профиль");
    m.insert(ADMIN, "Админ");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Пароль");
    m.insert(SIGN_IN, "Войти");
    m.insert(SIGN_OUT, "Выйти");
    m.insert(OFFLINE_MODE, "Офлайн-режим");
    m.insert(REFRESH, "Обновить");
    m.insert(FRIEND_LOCATIONS, "Местоположения друзей");
    m.insert(PUBLISH_MY_LOCATION, "Опубликовать моё местоположение");
    m.insert(LATITUDE, "Широта");
    m.insert(LONGITUDE, "Долгота");
    m.insert(PUBLISH, "Опубликовать");
    m.insert(NO_FRIENDS, "Друзей пока нет");
    m.insert(ACCEPT, "Принять");
    m.insert(REJECT, "Отклонить");
    m.insert(AWAITING_RESPONSE, "ожидание ответа");
    m.insert(LOCATION_SHARING, "Передача местоположения");
    m.insert(NO_SHARING, "Нет разрешений на передачу");
    m.insert(PAUSE, "Пауза");
    m.insert(RESUME, "Возобновить");
    m.insert(NAVIGATE_TO_FRIEND, "Перейти к другу");
    m.insert(NAVIGATE, "Перейти");
    m.insert(OSM_MAP, "Карта OSM");
    m.insert(NO_LOCATIONS, "Нет местоположений для навигации.");
    m.insert(SEARCH_PLACES, "Поиск мест (OpenStreetMap)");
    m.insert(SEARCH, "Поиск");
    m.insert(RESULTS, "Результаты");
    m.insert(LANGUAGE, "Язык");
    m.insert(PROFILE_VISIBILITY, "Видимость профиля");
    m.insert(FRIENDS_ONLY, "Только друзья");
    m.insert(PUBLIC, "Публичный");
    m.insert(SIGN_IN_REQUIRED, "Требуется вход");
    m.insert(
        SIGN_IN_TO_USE,
        "Чтобы использовать {feature}, сначала войдите.",
    );
    m.insert(GO_TO_MAP, "К карте");
    m.insert(
        OFFLINE_MAP_HINT,
        "Или останьтесь в режиме карты — она работает офлайн без входа.",
    );
    m
}

fn fr() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Carte");
    m.insert(FRIENDS, "Amis");
    m.insert(SHARING, "Partage");
    m.insert(PLACES, "Lieux");
    m.insert(SAVED, "Enregistrés");
    m.insert(TRIPS, "Voyages");
    m.insert(EMERGENCY, "Urgence");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Mot de passe");
    m.insert(SIGN_IN, "Se connecter");
    m.insert(SIGN_OUT, "Se déconnecter");
    m.insert(OFFLINE_MODE, "Mode hors ligne");
    m.insert(REFRESH, "Rafraîchir");
    m.insert(FRIEND_LOCATIONS, "Emplacements des amis");
    m.insert(PUBLISH_MY_LOCATION, "Publier ma position");
    m.insert(LATITUDE, "Latitude");
    m.insert(LONGITUDE, "Longitude");
    m.insert(PUBLISH, "Publier");
    m.insert(NO_FRIENDS, "Aucun ami pour le moment");
    m.insert(ACCEPT, "Accepter");
    m.insert(REJECT, "Refuser");
    m.insert(AWAITING_RESPONSE, "en attente de réponse");
    m.insert(LOCATION_SHARING, "Partage de position");
    m.insert(NO_SHARING, "Aucune autorisation de partage");
    m.insert(PAUSE, "Pause");
    m.insert(RESUME, "Reprendre");
    m.insert(NAVIGATE_TO_FRIEND, "Naviguer vers un ami");
    m.insert(NAVIGATE, "Naviguer");
    m.insert(OSM_MAP, "Carte OSM");
    m.insert(NO_LOCATIONS, "Aucun emplacement à naviguer.");
    m.insert(SEARCH_PLACES, "Rechercher des lieux (OpenStreetMap)");
    m.insert(SEARCH, "Rechercher");
    m.insert(RESULTS, "Résultats");
    m.insert(LANGUAGE, "Langue");
    m.insert(PROFILE_VISIBILITY, "Visibilité du profil");
    m.insert(FRIENDS_ONLY, "Amis uniquement");
    m.insert(PUBLIC, "Public");
    m.insert(SIGN_IN_REQUIRED, "Connexion requise");
    m.insert(
        SIGN_IN_TO_USE,
        "Pour utiliser {feature}, connectez-vous d'abord.",
    );
    m.insert(GO_TO_MAP, "Aller à la carte");
    m.insert(
        OFFLINE_MAP_HINT,
        "Ou restez en mode carte — elle fonctionne hors ligne sans connexion.",
    );
    m
}

fn de() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Karte");
    m.insert(FRIENDS, "Freunde");
    m.insert(SHARING, "Teilen");
    m.insert(PLACES, "Orte");
    m.insert(SAVED, "Gespeichert");
    m.insert(TRIPS, "Reisen");
    m.insert(EMERGENCY, "Notfall");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Passwort");
    m.insert(SIGN_IN, "Anmelden");
    m.insert(SIGN_OUT, "Abmelden");
    m.insert(OFFLINE_MODE, "Offline-Modus");
    m.insert(REFRESH, "Aktualisieren");
    m.insert(FRIEND_LOCATIONS, "Standorte der Freunde");
    m.insert(PUBLISH_MY_LOCATION, "Meinen Standort veröffentlichen");
    m.insert(LATITUDE, "Breitengrad");
    m.insert(LONGITUDE, "Längengrad");
    m.insert(PUBLISH, "Veröffentlichen");
    m.insert(NO_FRIENDS, "Noch keine Freunde");
    m.insert(ACCEPT, "Annehmen");
    m.insert(REJECT, "Ablehnen");
    m.insert(AWAITING_RESPONSE, "wartet auf Antwort");
    m.insert(LOCATION_SHARING, "Standortfreigabe");
    m.insert(NO_SHARING, "Keine Freigabeberechtigungen");
    m.insert(PAUSE, "Pause");
    m.insert(RESUME, "Fortsetzen");
    m.insert(NAVIGATE_TO_FRIEND, "Zum Freund navigieren");
    m.insert(NAVIGATE, "Navigieren");
    m.insert(OSM_MAP, "OSM-Karte");
    m.insert(NO_LOCATIONS, "Keine Standorte zum Navigieren.");
    m.insert(SEARCH_PLACES, "Orte suchen (OpenStreetMap)");
    m.insert(SEARCH, "Suchen");
    m.insert(RESULTS, "Ergebnisse");
    m.insert(LANGUAGE, "Sprache");
    m.insert(PROFILE_VISIBILITY, "Profil-Sichtbarkeit");
    m.insert(FRIENDS_ONLY, "Nur Freunde");
    m.insert(PUBLIC, "Öffentlich");
    m.insert(SIGN_IN_REQUIRED, "Anmeldung erforderlich");
    m.insert(
        SIGN_IN_TO_USE,
        "Um {feature} zu nutzen, melde dich zuerst an.",
    );
    m.insert(GO_TO_MAP, "Zur Karte");
    m.insert(
        OFFLINE_MAP_HINT,
        "Oder bleib im Kartenmodus — er funktioniert offline ohne Anmeldung.",
    );
    m
}

fn es() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Mapa");
    m.insert(FRIENDS, "Amigos");
    m.insert(SHARING, "Compartir");
    m.insert(PLACES, "Lugares");
    m.insert(SAVED, "Guardados");
    m.insert(TRIPS, "Viajes");
    m.insert(EMERGENCY, "Emergencia");
    m.insert(PROFILE, "Perfil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Contraseña");
    m.insert(SIGN_IN, "Iniciar sesión");
    m.insert(SIGN_OUT, "Cerrar sesión");
    m.insert(OFFLINE_MODE, "Modo sin conexión");
    m.insert(REFRESH, "Actualizar");
    m.insert(FRIEND_LOCATIONS, "Ubicaciones de amigos");
    m.insert(PUBLISH_MY_LOCATION, "Publicar mi ubicación");
    m.insert(LATITUDE, "Latitud");
    m.insert(LONGITUDE, "Longitud");
    m.insert(PUBLISH, "Publicar");
    m.insert(NO_FRIENDS, "Aún no hay amigos");
    m.insert(ACCEPT, "Aceptar");
    m.insert(REJECT, "Rechazar");
    m.insert(AWAITING_RESPONSE, "esperando respuesta");
    m.insert(LOCATION_SHARING, "Compartir ubicación");
    m.insert(NO_SHARING, "Sin permisos para compartir");
    m.insert(PAUSE, "Pausar");
    m.insert(RESUME, "Reanudar");
    m.insert(NAVIGATE_TO_FRIEND, "Ir a un amigo");
    m.insert(NAVIGATE, "Ir");
    m.insert(OSM_MAP, "Mapa OSM");
    m.insert(NO_LOCATIONS, "No hay ubicaciones para navegar.");
    m.insert(SEARCH_PLACES, "Buscar lugares (OpenStreetMap)");
    m.insert(SEARCH, "Buscar");
    m.insert(RESULTS, "Resultados");
    m.insert(LANGUAGE, "Idioma");
    m.insert(PROFILE_VISIBILITY, "Visibilidad del perfil");
    m.insert(FRIENDS_ONLY, "Solo amigos");
    m.insert(PUBLIC, "Público");
    m.insert(SIGN_IN_REQUIRED, "Inicio de sesión requerido");
    m.insert(
        SIGN_IN_TO_USE,
        "Para usar {feature}, inicia sesión primero.",
    );
    m.insert(GO_TO_MAP, "Ir al mapa");
    m.insert(
        OFFLINE_MAP_HINT,
        "O quédate en modo mapa: funciona sin conexión sin iniciar sesión.",
    );
    m
}

fn pt() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Mapa");
    m.insert(FRIENDS, "Amigos");
    m.insert(SHARING, "Partilha");
    m.insert(PLACES, "Locais");
    m.insert(SAVED, "Guardados");
    m.insert(TRIPS, "Viagens");
    m.insert(EMERGENCY, "Emergência");
    m.insert(PROFILE, "Perfil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Palavra-passe");
    m.insert(SIGN_IN, "Iniciar sessão");
    m.insert(SIGN_OUT, "Terminar sessão");
    m.insert(OFFLINE_MODE, "Modo offline");
    m.insert(REFRESH, "Atualizar");
    m.insert(FRIEND_LOCATIONS, "Localizações dos amigos");
    m.insert(PUBLISH_MY_LOCATION, "Publicar a minha localização");
    m.insert(LATITUDE, "Latitude");
    m.insert(LONGITUDE, "Longitude");
    m.insert(PUBLISH, "Publicar");
    m.insert(NO_FRIENDS, "Ainda sem amigos");
    m.insert(ACCEPT, "Aceitar");
    m.insert(REJECT, "Rejeitar");
    m.insert(AWAITING_RESPONSE, "a aguardar resposta");
    m.insert(LOCATION_SHARING, "Partilha de localização");
    m.insert(NO_SHARING, "Sem permissões de partilha");
    m.insert(PAUSE, "Pausar");
    m.insert(RESUME, "Retomar");
    m.insert(NAVIGATE_TO_FRIEND, "Navegar para um amigo");
    m.insert(NAVIGATE, "Navegar");
    m.insert(OSM_MAP, "Mapa OSM");
    m.insert(NO_LOCATIONS, "Sem localizações para navegar.");
    m.insert(SEARCH_PLACES, "Procurar locais (OpenStreetMap)");
    m.insert(SEARCH, "Procurar");
    m.insert(RESULTS, "Resultados");
    m.insert(LANGUAGE, "Idioma");
    m.insert(PROFILE_VISIBILITY, "Visibilidade do perfil");
    m.insert(FRIENDS_ONLY, "Apenas amigos");
    m.insert(PUBLIC, "Público");
    m.insert(SIGN_IN_REQUIRED, "Início de sessão obrigatório");
    m.insert(
        SIGN_IN_TO_USE,
        "Para usar {feature}, inicie sessão primeiro.",
    );
    m.insert(GO_TO_MAP, "Ir para o mapa");
    m.insert(
        OFFLINE_MAP_HINT,
        "Ou fique no modo mapa — funciona offline sem iniciar sessão.",
    );
    m
}

fn it() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Mappa");
    m.insert(FRIENDS, "Amici");
    m.insert(SHARING, "Condivisione");
    m.insert(PLACES, "Luoghi");
    m.insert(SAVED, "Salvati");
    m.insert(TRIPS, "Viaggi");
    m.insert(EMERGENCY, "Emergenza");
    m.insert(PROFILE, "Profilo");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Password");
    m.insert(SIGN_IN, "Accedi");
    m.insert(SIGN_OUT, "Esci");
    m.insert(OFFLINE_MODE, "Modalità offline");
    m.insert(REFRESH, "Aggiorna");
    m.insert(FRIEND_LOCATIONS, "Posizioni degli amici");
    m.insert(PUBLISH_MY_LOCATION, "Pubblica la mia posizione");
    m.insert(LATITUDE, "Latitudine");
    m.insert(LONGITUDE, "Longitudine");
    m.insert(PUBLISH, "Pubblica");
    m.insert(NO_FRIENDS, "Nessun amico ancora");
    m.insert(ACCEPT, "Accetta");
    m.insert(REJECT, "Rifiuta");
    m.insert(AWAITING_RESPONSE, "in attesa di risposta");
    m.insert(LOCATION_SHARING, "Condivisione posizione");
    m.insert(NO_SHARING, "Nessuna autorizzazione alla condivisione");
    m.insert(PAUSE, "Pausa");
    m.insert(RESUME, "Riprendi");
    m.insert(NAVIGATE_TO_FRIEND, "Naviga verso un amico");
    m.insert(NAVIGATE, "Naviga");
    m.insert(OSM_MAP, "Mappa OSM");
    m.insert(NO_LOCATIONS, "Nessuna posizione da navigare.");
    m.insert(SEARCH_PLACES, "Cerca luoghi (OpenStreetMap)");
    m.insert(SEARCH, "Cerca");
    m.insert(RESULTS, "Risultati");
    m.insert(LANGUAGE, "Lingua");
    m.insert(PROFILE_VISIBILITY, "Visibilità del profilo");
    m.insert(FRIENDS_ONLY, "Solo amici");
    m.insert(PUBLIC, "Pubblico");
    m.insert(SIGN_IN_REQUIRED, "Accesso richiesto");
    m.insert(SIGN_IN_TO_USE, "Per usare {feature}, accedi prima.");
    m.insert(GO_TO_MAP, "Vai alla mappa");
    m.insert(
        OFFLINE_MAP_HINT,
        "O resta in modalità mappa: funziona offline senza accesso.",
    );
    m
}

fn nl() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Kaart");
    m.insert(FRIENDS, "Vrienden");
    m.insert(SHARING, "Delen");
    m.insert(PLACES, "Plaatsen");
    m.insert(SAVED, "Opgeslagen");
    m.insert(TRIPS, "Reizen");
    m.insert(EMERGENCY, "Noodgeval");
    m.insert(PROFILE, "Profiel");
    m.insert(ADMIN, "Beheer");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Wachtwoord");
    m.insert(SIGN_IN, "Inloggen");
    m.insert(SIGN_OUT, "Uitloggen");
    m.insert(OFFLINE_MODE, "Offline modus");
    m.insert(REFRESH, "Vernieuwen");
    m.insert(FRIEND_LOCATIONS, "Locaties van vrienden");
    m.insert(PUBLISH_MY_LOCATION, "Mijn locatie publiceren");
    m.insert(LATITUDE, "Breedtegraad");
    m.insert(LONGITUDE, "Lengtegraad");
    m.insert(PUBLISH, "Publiceren");
    m.insert(NO_FRIENDS, "Nog geen vrienden");
    m.insert(ACCEPT, "Accepteren");
    m.insert(REJECT, "Afwijzen");
    m.insert(AWAITING_RESPONSE, "wacht op antwoord");
    m.insert(LOCATION_SHARING, "Locatie delen");
    m.insert(NO_SHARING, "Geen delen-machtigingen");
    m.insert(PAUSE, "Pauzeren");
    m.insert(RESUME, "Hervatten");
    m.insert(NAVIGATE_TO_FRIEND, "Navigeer naar een vriend");
    m.insert(NAVIGATE, "Navigeren");
    m.insert(OSM_MAP, "OSM-kaart");
    m.insert(NO_LOCATIONS, "Geen locaties om naar te navigeren.");
    m.insert(SEARCH_PLACES, "Plaatsen zoeken (OpenStreetMap)");
    m.insert(SEARCH, "Zoeken");
    m.insert(RESULTS, "Resultaten");
    m.insert(LANGUAGE, "Taal");
    m.insert(PROFILE_VISIBILITY, "Profielzichtbaarheid");
    m.insert(FRIENDS_ONLY, "Alleen vrienden");
    m.insert(PUBLIC, "Openbaar");
    m.insert(SIGN_IN_REQUIRED, "Inloggen vereist");
    m.insert(SIGN_IN_TO_USE, "Om {feature} te gebruken, log eerst in.");
    m.insert(GO_TO_MAP, "Naar de kaart");
    m.insert(
        OFFLINE_MAP_HINT,
        "Of blijf in kaartmodus: die werkt offline zonder inloggen.",
    );
    m
}

fn pl() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Mapa");
    m.insert(FRIENDS, "Znajomi");
    m.insert(SHARING, "Udostępnianie");
    m.insert(PLACES, "Miejsca");
    m.insert(SAVED, "Zapisane");
    m.insert(TRIPS, "Podróże");
    m.insert(EMERGENCY, "Alarm");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Hasło");
    m.insert(SIGN_IN, "Zaloguj się");
    m.insert(SIGN_OUT, "Wyloguj się");
    m.insert(OFFLINE_MODE, "Tryb offline");
    m.insert(REFRESH, "Odśwież");
    m.insert(FRIEND_LOCATIONS, "Lokalizacje znajomych");
    m.insert(PUBLISH_MY_LOCATION, "Opublikuj moją lokalizację");
    m.insert(LATITUDE, "Szerokość");
    m.insert(LONGITUDE, "Długość");
    m.insert(PUBLISH, "Opublikuj");
    m.insert(NO_FRIENDS, "Brak znajomych");
    m.insert(ACCEPT, "Akceptuj");
    m.insert(REJECT, "Odrzuć");
    m.insert(AWAITING_RESPONSE, "oczekiwanie na odpowiedź");
    m.insert(LOCATION_SHARING, "Udostępnianie lokalizacji");
    m.insert(NO_SHARING, "Brak uprawnień do udostępniania");
    m.insert(PAUSE, "Wstrzymaj");
    m.insert(RESUME, "Wznów");
    m.insert(NAVIGATE_TO_FRIEND, "Nawiguj do znajomego");
    m.insert(NAVIGATE, "Nawiguj");
    m.insert(OSM_MAP, "Mapa OSM");
    m.insert(NO_LOCATIONS, "Brak lokalizacji do nawigacji.");
    m.insert(SEARCH_PLACES, "Szukaj miejsc (OpenStreetMap)");
    m.insert(SEARCH, "Szukaj");
    m.insert(RESULTS, "Wyniki");
    m.insert(LANGUAGE, "Język");
    m.insert(PROFILE_VISIBILITY, "Widoczność profilu");
    m.insert(FRIENDS_ONLY, "Tylko znajomi");
    m.insert(PUBLIC, "Publiczny");
    m.insert(SIGN_IN_REQUIRED, "Wymagane logowanie");
    m.insert(
        SIGN_IN_TO_USE,
        "Aby korzystać z {feature}, najpierw się zaloguj.",
    );
    m.insert(GO_TO_MAP, "Przejdź do mapy");
    m.insert(
        OFFLINE_MAP_HINT,
        "Lub zostań w trybie mapy — działa offline bez logowania.",
    );
    m
}

fn uk() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Карта");
    m.insert(FRIENDS, "Друзі");
    m.insert(SHARING, "Передача");
    m.insert(PLACES, "Місця");
    m.insert(SAVED, "Збережені");
    m.insert(TRIPS, "Поїздки");
    m.insert(EMERGENCY, "Нагдопомога");
    m.insert(PROFILE, "Профіль");
    m.insert(ADMIN, "Адмін");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Пароль");
    m.insert(SIGN_IN, "Увійти");
    m.insert(SIGN_OUT, "Вийти");
    m.insert(OFFLINE_MODE, "Офлайн-режим");
    m.insert(REFRESH, "Оновити");
    m.insert(FRIEND_LOCATIONS, "Місця друзів");
    m.insert(PUBLISH_MY_LOCATION, "Опублікувати моє місце");
    m.insert(LATITUDE, "Широта");
    m.insert(LONGITUDE, "Довгота");
    m.insert(PUBLISH, "Опублікувати");
    m.insert(NO_FRIENDS, "Друзів ще немає");
    m.insert(ACCEPT, "Прийняти");
    m.insert(REJECT, "Відхилити");
    m.insert(AWAITING_RESPONSE, "очікування відповіді");
    m.insert(LOCATION_SHARING, "Передача місця");
    m.insert(NO_SHARING, "Немає дозволів на передачу");
    m.insert(PAUSE, "Пауза");
    m.insert(RESUME, "Відновити");
    m.insert(NAVIGATE_TO_FRIEND, "Перейти до друга");
    m.insert(NAVIGATE, "Перейти");
    m.insert(OSM_MAP, "Карта OSM");
    m.insert(NO_LOCATIONS, "Немає місць для навігації.");
    m.insert(SEARCH_PLACES, "Пошук місць (OpenStreetMap)");
    m.insert(SEARCH, "Пошук");
    m.insert(RESULTS, "Результати");
    m.insert(LANGUAGE, "Мова");
    m.insert(PROFILE_VISIBILITY, "Видимість профілю");
    m.insert(FRIENDS_ONLY, "Лише друзі");
    m.insert(PUBLIC, "Публічний");
    m.insert(SIGN_IN_REQUIRED, "Потрібен вхід");
    m.insert(
        SIGN_IN_TO_USE,
        "Щоб користуватися {feature}, спочатку увійдіть.",
    );
    m.insert(GO_TO_MAP, "До карти");
    m.insert(
        OFFLINE_MAP_HINT,
        "Або залишайтеся в режимі карти — він працює офлайн без входу.",
    );
    m
}

fn tr_tr() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Harita");
    m.insert(FRIENDS, "Arkadaşlar");
    m.insert(SHARING, "Paylaşım");
    m.insert(PLACES, "Yerler");
    m.insert(SAVED, "Kaydedilenler");
    m.insert(TRIPS, "Geziler");
    m.insert(EMERGENCY, "Acil Durum");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Yönetici");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Şifre");
    m.insert(SIGN_IN, "Giriş yap");
    m.insert(SIGN_OUT, "Çıkış yap");
    m.insert(OFFLINE_MODE, "Çevrimdışı mod");
    m.insert(REFRESH, "Yenile");
    m.insert(FRIEND_LOCATIONS, "Arkadaş konumları");
    m.insert(PUBLISH_MY_LOCATION, "Konumumu yayınla");
    m.insert(LATITUDE, "Enlem");
    m.insert(LONGITUDE, "Boylam");
    m.insert(PUBLISH, "Yayınla");
    m.insert(NO_FRIENDS, "Henüz arkadaş yok");
    m.insert(ACCEPT, "Kabul et");
    m.insert(REJECT, "Reddet");
    m.insert(AWAITING_RESPONSE, "yanıt bekleniyor");
    m.insert(LOCATION_SHARING, "Konum paylaşımı");
    m.insert(NO_SHARING, "Paylaşım izni yok");
    m.insert(PAUSE, "Duraklat");
    m.insert(RESUME, "Sürdür");
    m.insert(NAVIGATE_TO_FRIEND, "Bir arkadaşına git");
    m.insert(NAVIGATE, "Git");
    m.insert(OSM_MAP, "OSM Haritası");
    m.insert(NO_LOCATIONS, "Gidilecek konum yok.");
    m.insert(SEARCH_PLACES, "Yer ara (OpenStreetMap)");
    m.insert(SEARCH, "Ara");
    m.insert(RESULTS, "Sonuçlar");
    m.insert(LANGUAGE, "Dil");
    m.insert(PROFILE_VISIBILITY, "Profil görünürlüğü");
    m.insert(FRIENDS_ONLY, "Sadece arkadaşlar");
    m.insert(PUBLIC, "Herkese açık");
    m.insert(SIGN_IN_REQUIRED, "Giriş gerekli");
    m.insert(
        SIGN_IN_TO_USE,
        "{feature} özelliklerini kullanmak için önce giriş yapın.",
    );
    m.insert(GO_TO_MAP, "Haritaya git");
    m.insert(
        OFFLINE_MAP_HINT,
        "Ya da harita modunda kalın — giriş yapmadan çevrimdışı çalışır.",
    );
    m
}

fn ar() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "الخريطة");
    m.insert(FRIENDS, "الأصدقاء");
    m.insert(SHARING, "المشاركة");
    m.insert(PLACES, "الأماكن");
    m.insert(SAVED, "المحفوظات");
    m.insert(TRIPS, "الرحلات");
    m.insert(EMERGENCY, "طوارئ");
    m.insert(PROFILE, "الملف الشخصي");
    m.insert(ADMIN, "الإدارة");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "كلمة المرور");
    m.insert(SIGN_IN, "تسجيل الدخول");
    m.insert(SIGN_OUT, "تسجيل الخروج");
    m.insert(OFFLINE_MODE, "وضع عدم الاتصال");
    m.insert(REFRESH, "تحديث");
    m.insert(FRIEND_LOCATIONS, "مواقع الأصدقاء");
    m.insert(PUBLISH_MY_LOCATION, "نشر موقعي");
    m.insert(LATITUDE, "خط العرض");
    m.insert(LONGITUDE, "خط الطول");
    m.insert(PUBLISH, "نشر");
    m.insert(NO_FRIENDS, "لا يوجد أصدقاء بعد");
    m.insert(ACCEPT, "قبول");
    m.insert(REJECT, "رفض");
    m.insert(AWAITING_RESPONSE, "بانتظار الرد");
    m.insert(LOCATION_SHARING, "مشاركة الموقع");
    m.insert(NO_SHARING, "لا توجد أذونات مشاركة");
    m.insert(PAUSE, "إيقاف مؤقت");
    m.insert(RESUME, "استئناف");
    m.insert(NAVIGATE_TO_FRIEND, "الانتقال إلى صديق");
    m.insert(NAVIGATE, "الانتقال");
    m.insert(OSM_MAP, "خريطة OSM");
    m.insert(NO_LOCATIONS, "لا توجد مواقع للانتقال إليها.");
    m.insert(SEARCH_PLACES, "ابحث عن أماكن (OpenStreetMap)");
    m.insert(SEARCH, "بحث");
    m.insert(RESULTS, "النتائج");
    m.insert(LANGUAGE, "اللغة");
    m.insert(PROFILE_VISIBILITY, "ظهور الملف الشخصي");
    m.insert(FRIENDS_ONLY, "الأصدقاء فقط");
    m.insert(PUBLIC, "عام");
    m.insert(SIGN_IN_REQUIRED, "تسجيل الدخول مطلوب");
    m.insert(SIGN_IN_TO_USE, "لاستخدام {feature}، سجل الدخول أولاً.");
    m.insert(GO_TO_MAP, "اذهب إلى الخريطة");
    m.insert(
        OFFLINE_MAP_HINT,
        "أو ابقَ في وضع الخريطة — تعمل دون اتصال دون تسجيل الدخول.",
    );
    m
}

fn he() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "מפה");
    m.insert(FRIENDS, "חברים");
    m.insert(SHARING, "שיתוף");
    m.insert(PLACES, "מקומות");
    m.insert(SAVED, "שמורים");
    m.insert(TRIPS, "טיולים");
    m.insert(EMERGENCY, "חירום");
    m.insert(PROFILE, "פרופיל");
    m.insert(ADMIN, "ניהול");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "סיסמה");
    m.insert(SIGN_IN, "התחברות");
    m.insert(SIGN_OUT, "התנתקות");
    m.insert(OFFLINE_MODE, "מצב לא מקוון");
    m.insert(REFRESH, "רענן");
    m.insert(FRIEND_LOCATIONS, "מיקומי חברים");
    m.insert(PUBLISH_MY_LOCATION, "פרסם את המיקום שלי");
    m.insert(LATITUDE, "קו רוחב");
    m.insert(LONGITUDE, "קו אורך");
    m.insert(PUBLISH, "פרסם");
    m.insert(NO_FRIENDS, "אין עדיין חברים");
    m.insert(ACCEPT, "קבל");
    m.insert(REJECT, "דחה");
    m.insert(AWAITING_RESPONSE, "ממתין לתשובה");
    m.insert(LOCATION_SHARING, "שיתוף מיקום");
    m.insert(NO_SHARING, "אין הרשאות שיתוף");
    m.insert(PAUSE, "השהה");
    m.insert(RESUME, "המשך");
    m.insert(NAVIGATE_TO_FRIEND, "נווט לחבר");
    m.insert(NAVIGATE, "נווט");
    m.insert(OSM_MAP, "מפת OSM");
    m.insert(NO_LOCATIONS, "אין מיקומים לניווט.");
    m.insert(SEARCH_PLACES, "חפש מקומות (OpenStreetMap)");
    m.insert(SEARCH, "חיפוש");
    m.insert(RESULTS, "תוצאות");
    m.insert(LANGUAGE, "שפה");
    m.insert(PROFILE_VISIBILITY, "נראות פרופיל");
    m.insert(FRIENDS_ONLY, "חברים בלבד");
    m.insert(PUBLIC, "ציבורי");
    m.insert(SIGN_IN_REQUIRED, "נדרשת התחברות");
    m.insert(SIGN_IN_TO_USE, "כדי להשתמש ב-{feature}, התחבר תחילה.");
    m.insert(GO_TO_MAP, "עבור למפה");
    m.insert(
        OFFLINE_MAP_HINT,
        "או הישאר במצב מפה — היא עובדת ללא חיבור ללא התחברות.",
    );
    m
}

fn zh() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "地图");
    m.insert(FRIENDS, "好友");
    m.insert(SHARING, "共享");
    m.insert(PLACES, "地点");
    m.insert(SAVED, "已保存");
    m.insert(TRIPS, "行程");
    m.insert(EMERGENCY, "紧急");
    m.insert(PROFILE, "个人资料");
    m.insert(ADMIN, "管理");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "密码");
    m.insert(SIGN_IN, "登录");
    m.insert(SIGN_OUT, "退出登录");
    m.insert(OFFLINE_MODE, "离线模式");
    m.insert(REFRESH, "刷新");
    m.insert(FRIEND_LOCATIONS, "好友位置");
    m.insert(PUBLISH_MY_LOCATION, "发布我的位置");
    m.insert(LATITUDE, "纬度");
    m.insert(LONGITUDE, "经度");
    m.insert(PUBLISH, "发布");
    m.insert(NO_FRIENDS, "还没有好友");
    m.insert(ACCEPT, "接受");
    m.insert(REJECT, "拒绝");
    m.insert(AWAITING_RESPONSE, "等待回复");
    m.insert(LOCATION_SHARING, "位置共享");
    m.insert(NO_SHARING, "没有共享权限");
    m.insert(PAUSE, "暂停");
    m.insert(RESUME, "恢复");
    m.insert(NAVIGATE_TO_FRIEND, "导航到好友");
    m.insert(NAVIGATE, "导航");
    m.insert(OSM_MAP, "OSM 地图");
    m.insert(NO_LOCATIONS, "没有可导航的位置。");
    m.insert(SEARCH_PLACES, "搜索地点（OpenStreetMap）");
    m.insert(SEARCH, "搜索");
    m.insert(RESULTS, "结果");
    m.insert(LANGUAGE, "语言");
    m.insert(PROFILE_VISIBILITY, "个人资料可见性");
    m.insert(FRIENDS_ONLY, "仅好友");
    m.insert(PUBLIC, "公开");
    m.insert(SIGN_IN_REQUIRED, "需要登录");
    m.insert(SIGN_IN_TO_USE, "要使用{feature}，请先登录。");
    m.insert(GO_TO_MAP, "前往地图");
    m.insert(OFFLINE_MAP_HINT, "或留在地图模式——无需登录即可离线使用。");
    m
}

fn ja() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "地図");
    m.insert(FRIENDS, "友達");
    m.insert(SHARING, "共有");
    m.insert(PLACES, "場所");
    m.insert(SAVED, "保存済み");
    m.insert(TRIPS, "旅行");
    m.insert(EMERGENCY, "緊急");
    m.insert(PROFILE, "プロフィール");
    m.insert(ADMIN, "管理");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "パスワード");
    m.insert(SIGN_IN, "ログイン");
    m.insert(SIGN_OUT, "ログアウト");
    m.insert(OFFLINE_MODE, "オフライン モード");
    m.insert(REFRESH, "更新");
    m.insert(FRIEND_LOCATIONS, "友達の位置");
    m.insert(PUBLISH_MY_LOCATION, "自分の位置を公開");
    m.insert(LATITUDE, "緯度");
    m.insert(LONGITUDE, "経度");
    m.insert(PUBLISH, "公開");
    m.insert(NO_FRIENDS, "まだ友達がいません");
    m.insert(ACCEPT, "承認");
    m.insert(REJECT, "拒否");
    m.insert(AWAITING_RESPONSE, "返信待ち");
    m.insert(LOCATION_SHARING, "位置共有");
    m.insert(NO_SHARING, "共有権限がありません");
    m.insert(PAUSE, "一時停止");
    m.insert(RESUME, "再開");
    m.insert(NAVIGATE_TO_FRIEND, "友達へ移動");
    m.insert(NAVIGATE, "移動");
    m.insert(OSM_MAP, "OSM 地図");
    m.insert(NO_LOCATIONS, "移動できる場所がありません。");
    m.insert(SEARCH_PLACES, "場所を検索（OpenStreetMap）");
    m.insert(SEARCH, "検索");
    m.insert(RESULTS, "結果");
    m.insert(LANGUAGE, "言語");
    m.insert(PROFILE_VISIBILITY, "プロフィールの公開範囲");
    m.insert(FRIENDS_ONLY, "友達のみ");
    m.insert(PUBLIC, "公開");
    m.insert(SIGN_IN_REQUIRED, "ログインが必要です");
    m.insert(
        SIGN_IN_TO_USE,
        "{feature}を使うには、まずログインしてください。",
    );
    m.insert(GO_TO_MAP, "地図へ");
    m.insert(
        OFFLINE_MAP_HINT,
        "または地図モードのままで——ログインなしでオフラインで使えます。",
    );
    m
}

fn ko() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "지도");
    m.insert(FRIENDS, "친구");
    m.insert(SHARING, "공유");
    m.insert(PLACES, "장소");
    m.insert(SAVED, "저장됨");
    m.insert(TRIPS, "여행");
    m.insert(EMERGENCY, "긴급");
    m.insert(PROFILE, "프로필");
    m.insert(ADMIN, "관리자");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "비밀번호");
    m.insert(SIGN_IN, "로그인");
    m.insert(SIGN_OUT, "로그아웃");
    m.insert(OFFLINE_MODE, "오프라인 모드");
    m.insert(REFRESH, "새로 고침");
    m.insert(FRIEND_LOCATIONS, "친구 위치");
    m.insert(PUBLISH_MY_LOCATION, "내 위치 게시");
    m.insert(LATITUDE, "위도");
    m.insert(LONGITUDE, "경도");
    m.insert(PUBLISH, "게시");
    m.insert(NO_FRIENDS, "아직 친구가 없습니다");
    m.insert(ACCEPT, "수락");
    m.insert(REJECT, "거절");
    m.insert(AWAITING_RESPONSE, "응답 대기 중");
    m.insert(LOCATION_SHARING, "위치 공유");
    m.insert(NO_SHARING, "공유 권한이 없습니다");
    m.insert(PAUSE, "일시 중지");
    m.insert(RESUME, "재개");
    m.insert(NAVIGATE_TO_FRIEND, "친구에게 이동");
    m.insert(NAVIGATE, "이동");
    m.insert(OSM_MAP, "OSM 지도");
    m.insert(NO_LOCATIONS, "이동할 위치가 없습니다.");
    m.insert(SEARCH_PLACES, "장소 검색(OpenStreetMap)");
    m.insert(SEARCH, "검색");
    m.insert(RESULTS, "결과");
    m.insert(LANGUAGE, "언어");
    m.insert(PROFILE_VISIBILITY, "프로필 공개 범위");
    m.insert(FRIENDS_ONLY, "친구만");
    m.insert(PUBLIC, "공개");
    m.insert(SIGN_IN_REQUIRED, "로그인 필요");
    m.insert(
        SIGN_IN_TO_USE,
        "{feature}을(를) 사용하려면 먼저 로그인하세요.",
    );
    m.insert(GO_TO_MAP, "지도로 이동");
    m.insert(
        OFFLINE_MAP_HINT,
        "또는 지도 모드로 유지하세요 — 로그인 없이 오프라인으로 작동합니다.",
    );
    m
}

fn hi() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "नक्शा");
    m.insert(FRIENDS, "दोस्त");
    m.insert(SHARING, "साझा करना");
    m.insert(PLACES, "स्थान");
    m.insert(SAVED, "सहेजे गए");
    m.insert(TRIPS, "यात्राएँ");
    m.insert(EMERGENCY, "आपातकाल");
    m.insert(PROFILE, "प्रोफ़ाइल");
    m.insert(ADMIN, "एडमिन");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "पासवर्ड");
    m.insert(SIGN_IN, "साइन इन");
    m.insert(SIGN_OUT, "साइन आउट");
    m.insert(OFFLINE_MODE, "ऑफ़लाइन मोड");
    m.insert(REFRESH, "रीफ़्रेश");
    m.insert(FRIEND_LOCATIONS, "दोस्तों के स्थान");
    m.insert(PUBLISH_MY_LOCATION, "मेरा स्थान प्रकाशित करें");
    m.insert(LATITUDE, "अक्षांश");
    m.insert(LONGITUDE, "देशांतर");
    m.insert(PUBLISH, "प्रकाशित करें");
    m.insert(NO_FRIENDS, "अभी कोई दोस्त नहीं");
    m.insert(ACCEPT, "स्वीकार करें");
    m.insert(REJECT, "अस्वीकार करें");
    m.insert(AWAITING_RESPONSE, "उत्तर की प्रतीक्षा");
    m.insert(LOCATION_SHARING, "स्थान साझा करना");
    m.insert(NO_SHARING, "कोई साझा करने की अनुमति नहीं");
    m.insert(PAUSE, "रोकें");
    m.insert(RESUME, "फिर से शुरू करें");
    m.insert(NAVIGATE_TO_FRIEND, "किसी दोस्त की ओर जाएँ");
    m.insert(NAVIGATE, "नेविगेट");
    m.insert(OSM_MAP, "OSM नक्शा");
    m.insert(NO_LOCATIONS, "नेविगेट करने के लिए कोई स्थान नहीं।");
    m.insert(SEARCH_PLACES, "स्थान खोजें (OpenStreetMap)");
    m.insert(SEARCH, "खोजें");
    m.insert(RESULTS, "परिणाम");
    m.insert(LANGUAGE, "भाषा");
    m.insert(PROFILE_VISIBILITY, "प्रोफ़ाइल दृश्यता");
    m.insert(FRIENDS_ONLY, "केवल दोस्त");
    m.insert(PUBLIC, "सार्वजनिक");
    m.insert(SIGN_IN_REQUIRED, "साइन इन आवश्यक");
    m.insert(
        SIGN_IN_TO_USE,
        "{feature} का उपयोग करने के लिए पहले साइन इन करें।",
    );
    m.insert(GO_TO_MAP, "नक्शे पर जाएँ");
    m.insert(
        OFFLINE_MAP_HINT,
        "या नक्शा मोड में रहें — साइन इन के बिना ऑफ़लाइन काम करता है।",
    );
    m
}

fn vi() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Bản đồ");
    m.insert(FRIENDS, "Bạn bè");
    m.insert(SHARING, "Chia sẻ");
    m.insert(PLACES, "Địa điểm");
    m.insert(SAVED, "Đã lưu");
    m.insert(TRIPS, "Chuyến đi");
    m.insert(EMERGENCY, "Khẩn cấp");
    m.insert(PROFILE, "Hồ sơ");
    m.insert(ADMIN, "Quản trị");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Mật khẩu");
    m.insert(SIGN_IN, "Đăng nhập");
    m.insert(SIGN_OUT, "Đăng xuất");
    m.insert(OFFLINE_MODE, "Chế độ ngoại tuyến");
    m.insert(REFRESH, "Làm mới");
    m.insert(FRIEND_LOCATIONS, "Vị trí bạn bè");
    m.insert(PUBLISH_MY_LOCATION, "Công bố vị trí của tôi");
    m.insert(LATITUDE, "Vĩ độ");
    m.insert(LONGITUDE, "Kinh độ");
    m.insert(PUBLISH, "Công bố");
    m.insert(NO_FRIENDS, "Chưa có bạn bè");
    m.insert(ACCEPT, "Chấp nhận");
    m.insert(REJECT, "Từ chối");
    m.insert(AWAITING_RESPONSE, "đang chờ phản hồi");
    m.insert(LOCATION_SHARING, "Chia sẻ vị trí");
    m.insert(NO_SHARING, "Không có quyền chia sẻ");
    m.insert(PAUSE, "Tạm dừng");
    m.insert(RESUME, "Tiếp tục");
    m.insert(NAVIGATE_TO_FRIEND, "Đi đến bạn bè");
    m.insert(NAVIGATE, "Điều hướng");
    m.insert(OSM_MAP, "Bản đồ OSM");
    m.insert(NO_LOCATIONS, "Không có vị trí để điều hướng.");
    m.insert(SEARCH_PLACES, "Tìm địa điểm (OpenStreetMap)");
    m.insert(SEARCH, "Tìm kiếm");
    m.insert(RESULTS, "Kết quả");
    m.insert(LANGUAGE, "Ngôn ngữ");
    m.insert(PROFILE_VISIBILITY, "Hiển thị hồ sơ");
    m.insert(FRIENDS_ONLY, "Chỉ bạn bè");
    m.insert(PUBLIC, "Công khai");
    m.insert(SIGN_IN_REQUIRED, "Yêu cầu đăng nhập");
    m.insert(
        SIGN_IN_TO_USE,
        "Để dùng {feature}, vui lòng đăng nhập trước.",
    );
    m.insert(GO_TO_MAP, "Đến bản đồ");
    m.insert(
        OFFLINE_MAP_HINT,
        "Hoặc ở lại chế độ bản đồ — hoạt động ngoại tuyến không cần đăng nhập.",
    );
    m
}

fn th() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "แผนที่");
    m.insert(FRIENDS, "เพื่อน");
    m.insert(SHARING, "การแชร์");
    m.insert(PLACES, "สถานที่");
    m.insert(SAVED, "บันทึกแล้ว");
    m.insert(TRIPS, "การเดินทาง");
    m.insert(EMERGENCY, "ฉุกเฉิน");
    m.insert(PROFILE, "โปรไฟล์");
    m.insert(ADMIN, "ผู้ดูแล");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "รหัสผ่าน");
    m.insert(SIGN_IN, "เข้าสู่ระบบ");
    m.insert(SIGN_OUT, "ออกจากระบบ");
    m.insert(OFFLINE_MODE, "โหมดออฟไลน์");
    m.insert(REFRESH, "รีเฟรช");
    m.insert(FRIEND_LOCATIONS, "ตำแหน่งเพื่อน");
    m.insert(PUBLISH_MY_LOCATION, "เผยแพร่ตำแหน่งของฉัน");
    m.insert(LATITUDE, "ละติจูด");
    m.insert(LONGITUDE, "ลองจิจูด");
    m.insert(PUBLISH, "เผยแพร่");
    m.insert(NO_FRIENDS, "ยังไม่มีเพื่อน");
    m.insert(ACCEPT, "ยอมรับ");
    m.insert(REJECT, "ปฏิเสธ");
    m.insert(AWAITING_RESPONSE, "กำลังรอการตอบกลับ");
    m.insert(LOCATION_SHARING, "การแชร์ตำแหน่ง");
    m.insert(NO_SHARING, "ไม่มีสิทธิ์แชร์");
    m.insert(PAUSE, "หยุดชั่วคราว");
    m.insert(RESUME, "ดำเนินการต่อ");
    m.insert(NAVIGATE_TO_FRIEND, "นำทางไปหาเพื่อน");
    m.insert(NAVIGATE, "นำทาง");
    m.insert(OSM_MAP, "แผนที่ OSM");
    m.insert(NO_LOCATIONS, "ไม่มีตำแหน่งให้นำทาง");
    m.insert(SEARCH_PLACES, "ค้นหาสถานที่ (OpenStreetMap)");
    m.insert(SEARCH, "ค้นหา");
    m.insert(RESULTS, "ผลลัพธ์");
    m.insert(LANGUAGE, "ภาษา");
    m.insert(PROFILE_VISIBILITY, "การมองเห็นโปรไฟล์");
    m.insert(FRIENDS_ONLY, "เพื่อนเท่านั้น");
    m.insert(PUBLIC, "สาธารณะ");
    m.insert(SIGN_IN_REQUIRED, "ต้องเข้าสู่ระบบ");
    m.insert(SIGN_IN_TO_USE, "เพื่อใช้ {feature} กรุณาเข้าสู่ระบบก่อน");
    m.insert(GO_TO_MAP, "ไปที่แผนที่");
    m.insert(
        OFFLINE_MAP_HINT,
        "หรืออยู่ในโหมดแผนที่ — ใช้งานออฟไลน์ได้โดยไม่ต้องเข้าสู่ระบบ",
    );
    m
}

fn id() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Peta");
    m.insert(FRIENDS, "Teman");
    m.insert(SHARING, "Berbagi");
    m.insert(PLACES, "Tempat");
    m.insert(SAVED, "Tersimpan");
    m.insert(TRIPS, "Perjalanan");
    m.insert(EMERGENCY, "Darurat");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Kata sandi");
    m.insert(SIGN_IN, "Masuk");
    m.insert(SIGN_OUT, "Keluar");
    m.insert(OFFLINE_MODE, "Mode offline");
    m.insert(REFRESH, "Segarkan");
    m.insert(FRIEND_LOCATIONS, "Lokasi teman");
    m.insert(PUBLISH_MY_LOCATION, "Publikasikan lokasi saya");
    m.insert(LATITUDE, "Garis lintang");
    m.insert(LONGITUDE, "Garis bujur");
    m.insert(PUBLISH, "Publikasikan");
    m.insert(NO_FRIENDS, "Belum ada teman");
    m.insert(ACCEPT, "Terima");
    m.insert(REJECT, "Tolak");
    m.insert(AWAITING_RESPONSE, "menunggu tanggapan");
    m.insert(LOCATION_SHARING, "Berbagi lokasi");
    m.insert(NO_SHARING, "Tidak ada izin berbagi");
    m.insert(PAUSE, "Jeda");
    m.insert(RESUME, "Lanjutkan");
    m.insert(NAVIGATE_TO_FRIEND, "Navigasi ke teman");
    m.insert(NAVIGATE, "Navigasi");
    m.insert(OSM_MAP, "Peta OSM");
    m.insert(NO_LOCATIONS, "Tidak ada lokasi untuk dinavigasi.");
    m.insert(SEARCH_PLACES, "Cari tempat (OpenStreetMap)");
    m.insert(SEARCH, "Cari");
    m.insert(RESULTS, "Hasil");
    m.insert(LANGUAGE, "Bahasa");
    m.insert(PROFILE_VISIBILITY, "Visibilitas profil");
    m.insert(FRIENDS_ONLY, "Hanya teman");
    m.insert(PUBLIC, "Publik");
    m.insert(SIGN_IN_REQUIRED, "Perlu masuk");
    m.insert(
        SIGN_IN_TO_USE,
        "Untuk menggunakan {feature}, masuk terlebih dahulu.",
    );
    m.insert(GO_TO_MAP, "Ke peta");
    m.insert(
        OFFLINE_MAP_HINT,
        "Atau tetap di mode peta — berfungsi offline tanpa masuk.",
    );
    m
}

fn el() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Χάρτης");
    m.insert(FRIENDS, "Φίλοι");
    m.insert(SHARING, "Κοινοποίηση");
    m.insert(PLACES, "Μέρη");
    m.insert(SAVED, "Αποθηκευμένα");
    m.insert(TRIPS, "Ταξίδια");
    m.insert(EMERGENCY, "Έκτακτη ανάγκη");
    m.insert(PROFILE, "Προφίλ");
    m.insert(ADMIN, "Διαχείριση");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Κωδικός");
    m.insert(SIGN_IN, "Σύνδεση");
    m.insert(SIGN_OUT, "Αποσύνδεση");
    m.insert(OFFLINE_MODE, "Λειτουργία εκτός σύνδεσης");
    m.insert(REFRESH, "Ανανέωση");
    m.insert(FRIEND_LOCATIONS, "Τοποθεσίες φίλων");
    m.insert(PUBLISH_MY_LOCATION, "Δημοσίευση της τοποθεσίας μου");
    m.insert(LATITUDE, "Γεωγραφικό πλάτος");
    m.insert(LONGITUDE, "Γεωγραφικό μήκος");
    m.insert(PUBLISH, "Δημοσίευση");
    m.insert(NO_FRIENDS, "Δεν υπάρχουν φίλοι ακόμα");
    m.insert(ACCEPT, "Αποδοχή");
    m.insert(REJECT, "Απόρριψη");
    m.insert(AWAITING_RESPONSE, "αναμονή απάντησης");
    m.insert(LOCATION_SHARING, "Κοινοποίηση τοποθεσίας");
    m.insert(NO_SHARING, "Δεν υπάρχουν δικαιώματα κοινοποίησης");
    m.insert(PAUSE, "Παύση");
    m.insert(RESUME, "Συνέχεια");
    m.insert(NAVIGATE_TO_FRIEND, "Πλοήγηση σε φίλο");
    m.insert(NAVIGATE, "Πλοήγηση");
    m.insert(OSM_MAP, "Χάρτης OSM");
    m.insert(NO_LOCATIONS, "Δεν υπάρχουν τοποθεσίες για πλοήγηση.");
    m.insert(SEARCH_PLACES, "Αναζήτηση μερών (OpenStreetMap)");
    m.insert(SEARCH, "Αναζήτηση");
    m.insert(RESULTS, "Αποτελέσματα");
    m.insert(LANGUAGE, "Γλώσσα");
    m.insert(PROFILE_VISIBILITY, "Ορατότητα προφίλ");
    m.insert(FRIENDS_ONLY, "Μόνο φίλοι");
    m.insert(PUBLIC, "Δημόσιο");
    m.insert(SIGN_IN_REQUIRED, "Απαιτείται σύνδεση");
    m.insert(
        SIGN_IN_TO_USE,
        "Για να χρησιμοποιήσετε {feature}, συνδεθείτε πρώτα.",
    );
    m.insert(GO_TO_MAP, "Μετάβαση στον χάρτη");
    m.insert(
        OFFLINE_MAP_HINT,
        "Ή μείνετε σε λειτουργία χάρτη — λειτουργεί εκτός σύνδεσης χωρίς σύνδεση.",
    );
    m
}

fn cs() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Mapa");
    m.insert(FRIENDS, "Přátelé");
    m.insert(SHARING, "Sdílení");
    m.insert(PLACES, "Místa");
    m.insert(SAVED, "Uložené");
    m.insert(TRIPS, "Cesty");
    m.insert(EMERGENCY, "Nouze");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Správa");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Heslo");
    m.insert(SIGN_IN, "Přihlásit se");
    m.insert(SIGN_OUT, "Odhlásit se");
    m.insert(OFFLINE_MODE, "Režim offline");
    m.insert(REFRESH, "Obnovit");
    m.insert(FRIEND_LOCATIONS, "Poloha přátel");
    m.insert(PUBLISH_MY_LOCATION, "Zveřejnit moji polohu");
    m.insert(LATITUDE, "Zeměpisná šířka");
    m.insert(LONGITUDE, "Zeměpisná délka");
    m.insert(PUBLISH, "Zveřejnit");
    m.insert(NO_FRIENDS, "Zatím žádní přátelé");
    m.insert(ACCEPT, "Přijmout");
    m.insert(REJECT, "Odmítnout");
    m.insert(AWAITING_RESPONSE, "čeká se na odpověď");
    m.insert(LOCATION_SHARING, "Sdílení polohy");
    m.insert(NO_SHARING, "Žádná oprávnění ke sdílení");
    m.insert(PAUSE, "Pozastavit");
    m.insert(RESUME, "Obnovit");
    m.insert(NAVIGATE_TO_FRIEND, "Navigovat k příteli");
    m.insert(NAVIGATE, "Navigovat");
    m.insert(OSM_MAP, "Mapa OSM");
    m.insert(NO_LOCATIONS, "Žádná místa k navigaci.");
    m.insert(SEARCH_PLACES, "Hledat místa (OpenStreetMap)");
    m.insert(SEARCH, "Hledat");
    m.insert(RESULTS, "Výsledky");
    m.insert(LANGUAGE, "Jazyk");
    m.insert(PROFILE_VISIBILITY, "Viditelnost profilu");
    m.insert(FRIENDS_ONLY, "Jen přátelé");
    m.insert(PUBLIC, "Veřejné");
    m.insert(SIGN_IN_REQUIRED, "Vyžadováno přihlášení");
    m.insert(
        SIGN_IN_TO_USE,
        "Pro použití {feature} se nejprve přihlaste.",
    );
    m.insert(GO_TO_MAP, "Přejít na mapu");
    m.insert(
        OFFLINE_MAP_HINT,
        "Nebo zůstaňte v režimu mapy — funguje offline bez přihlášení.",
    );
    m
}

fn hu() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Térkép");
    m.insert(FRIENDS, "Barátok");
    m.insert(SHARING, "Megosztás");
    m.insert(PLACES, "Helyek");
    m.insert(SAVED, "Mentett");
    m.insert(TRIPS, "Utazások");
    m.insert(EMERGENCY, "Vészhelyzet");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Jelszó");
    m.insert(SIGN_IN, "Bejelentkezés");
    m.insert(SIGN_OUT, "Kijelentkezés");
    m.insert(OFFLINE_MODE, "Offline mód");
    m.insert(REFRESH, "Frissítés");
    m.insert(FRIEND_LOCATIONS, "Barátok helye");
    m.insert(PUBLISH_MY_LOCATION, "Saját hely közzététele");
    m.insert(LATITUDE, "Szélesség");
    m.insert(LONGITUDE, "Hosszúság");
    m.insert(PUBLISH, "Közzététel");
    m.insert(NO_FRIENDS, "Még nincsenek barátok");
    m.insert(ACCEPT, "Elfogadás");
    m.insert(REJECT, "Elutasítás");
    m.insert(AWAITING_RESPONSE, "várakozás válaszra");
    m.insert(LOCATION_SHARING, "Helymegosztás");
    m.insert(NO_SHARING, "Nincs megosztási engedély");
    m.insert(PAUSE, "Szünet");
    m.insert(RESUME, "Folytatás");
    m.insert(NAVIGATE_TO_FRIEND, "Navigálás baráthoz");
    m.insert(NAVIGATE, "Navigálás");
    m.insert(OSM_MAP, "OSM térkép");
    m.insert(NO_LOCATIONS, "Nincs navigálható hely.");
    m.insert(SEARCH_PLACES, "Helyek keresése (OpenStreetMap)");
    m.insert(SEARCH, "Keresés");
    m.insert(RESULTS, "Eredmények");
    m.insert(LANGUAGE, "Nyelv");
    m.insert(PROFILE_VISIBILITY, "Profil láthatósága");
    m.insert(FRIENDS_ONLY, "Csak barátok");
    m.insert(PUBLIC, "Nyilvános");
    m.insert(SIGN_IN_REQUIRED, "Bejelentkezés szükséges");
    m.insert(
        SIGN_IN_TO_USE,
        "A {feature} használatához először jelentkezzen be.",
    );
    m.insert(GO_TO_MAP, "Ugrás a térképre");
    m.insert(
        OFFLINE_MAP_HINT,
        "Vagy maradjon térkép módban — bejelentkezés nélkül offline működik.",
    );
    m
}

fn sv() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Karta");
    m.insert(FRIENDS, "Vänner");
    m.insert(SHARING, "Delning");
    m.insert(PLACES, "Platser");
    m.insert(SAVED, "Sparade");
    m.insert(TRIPS, "Resor");
    m.insert(EMERGENCY, "Nödsituation");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Lösenord");
    m.insert(SIGN_IN, "Logga in");
    m.insert(SIGN_OUT, "Logga ut");
    m.insert(OFFLINE_MODE, "Offline-läge");
    m.insert(REFRESH, "Uppdatera");
    m.insert(FRIEND_LOCATIONS, "Vänners platser");
    m.insert(PUBLISH_MY_LOCATION, "Publicera min plats");
    m.insert(LATITUDE, "Latitud");
    m.insert(LONGITUDE, "Longitud");
    m.insert(PUBLISH, "Publicera");
    m.insert(NO_FRIENDS, "Inga vänner ännu");
    m.insert(ACCEPT, "Acceptera");
    m.insert(REJECT, "Avvisa");
    m.insert(AWAITING_RESPONSE, "väntar på svar");
    m.insert(LOCATION_SHARING, "Platsdelning");
    m.insert(NO_SHARING, "Inga delningsbehörigheter");
    m.insert(PAUSE, "Paus");
    m.insert(RESUME, "Återuppta");
    m.insert(NAVIGATE_TO_FRIEND, "Navigera till en vän");
    m.insert(NAVIGATE, "Navigera");
    m.insert(OSM_MAP, "OSM-karta");
    m.insert(NO_LOCATIONS, "Inga platser att navigera till.");
    m.insert(SEARCH_PLACES, "Sök platser (OpenStreetMap)");
    m.insert(SEARCH, "Sök");
    m.insert(RESULTS, "Resultat");
    m.insert(LANGUAGE, "Språk");
    m.insert(PROFILE_VISIBILITY, "Profilens synlighet");
    m.insert(FRIENDS_ONLY, "Endast vänner");
    m.insert(PUBLIC, "Offentlig");
    m.insert(SIGN_IN_REQUIRED, "Inloggning krävs");
    m.insert(SIGN_IN_TO_USE, "För att använda {feature}, logga in först.");
    m.insert(GO_TO_MAP, "Gå till kartan");
    m.insert(
        OFFLINE_MAP_HINT,
        "Eller stanna i kartläge — det fungerar offline utan inloggning.",
    );
    m
}

fn no() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Kart");
    m.insert(FRIENDS, "Venner");
    m.insert(SHARING, "Deling");
    m.insert(PLACES, "Steder");
    m.insert(SAVED, "Lagrede");
    m.insert(TRIPS, "Reiser");
    m.insert(EMERGENCY, "Nødsituasjon");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Passord");
    m.insert(SIGN_IN, "Logg inn");
    m.insert(SIGN_OUT, "Logg ut");
    m.insert(OFFLINE_MODE, "Offline-modus");
    m.insert(REFRESH, "Oppdater");
    m.insert(FRIEND_LOCATIONS, "Venners steder");
    m.insert(PUBLISH_MY_LOCATION, "Publiser min posisjon");
    m.insert(LATITUDE, "Breddegrad");
    m.insert(LONGITUDE, "Lengdegrad");
    m.insert(PUBLISH, "Publiser");
    m.insert(NO_FRIENDS, "Ingen venner ennå");
    m.insert(ACCEPT, "Godta");
    m.insert(REJECT, "Avvis");
    m.insert(AWAITING_RESPONSE, "venter på svar");
    m.insert(LOCATION_SHARING, "Posisjonsdeling");
    m.insert(NO_SHARING, "Ingen delingsrettigheter");
    m.insert(PAUSE, "Pause");
    m.insert(RESUME, "Fortsett");
    m.insert(NAVIGATE_TO_FRIEND, "Naviger til en venn");
    m.insert(NAVIGATE, "Naviger");
    m.insert(OSM_MAP, "OSM-kart");
    m.insert(NO_LOCATIONS, "Ingen steder å navigere til.");
    m.insert(SEARCH_PLACES, "Søk etter steder (OpenStreetMap)");
    m.insert(SEARCH, "Søk");
    m.insert(RESULTS, "Resultater");
    m.insert(LANGUAGE, "Språk");
    m.insert(PROFILE_VISIBILITY, "Profilens synlighet");
    m.insert(FRIENDS_ONLY, "Bare venner");
    m.insert(PUBLIC, "Offentlig");
    m.insert(SIGN_IN_REQUIRED, "Innlogging kreves");
    m.insert(SIGN_IN_TO_USE, "For å bruke {feature}, logg inn først.");
    m.insert(GO_TO_MAP, "Gå til kartet");
    m.insert(
        OFFLINE_MAP_HINT,
        "Eller bli i kartmodus — den fungerer offline uten innlogging.",
    );
    m
}

fn da() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Kort");
    m.insert(FRIENDS, "Venner");
    m.insert(SHARING, "Deling");
    m.insert(PLACES, "Steder");
    m.insert(SAVED, "Gemte");
    m.insert(TRIPS, "Rejser");
    m.insert(EMERGENCY, "Nødsituation");
    m.insert(PROFILE, "Profil");
    m.insert(ADMIN, "Admin");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Adgangskode");
    m.insert(SIGN_IN, "Log ind");
    m.insert(SIGN_OUT, "Log ud");
    m.insert(OFFLINE_MODE, "Offline-tilstand");
    m.insert(REFRESH, "Opdater");
    m.insert(FRIEND_LOCATIONS, "Venners placeringer");
    m.insert(PUBLISH_MY_LOCATION, "Offentliggør min placering");
    m.insert(LATITUDE, "Breddegrad");
    m.insert(LONGITUDE, "Længdegrad");
    m.insert(PUBLISH, "Offentliggør");
    m.insert(NO_FRIENDS, "Ingen venner endnu");
    m.insert(ACCEPT, "Accepter");
    m.insert(REJECT, "Afvis");
    m.insert(AWAITING_RESPONSE, "venter på svar");
    m.insert(LOCATION_SHARING, "Placeringsdeling");
    m.insert(NO_SHARING, "Ingen delingstilladelser");
    m.insert(PAUSE, "Pause");
    m.insert(RESUME, "Genoptag");
    m.insert(NAVIGATE_TO_FRIEND, "Naviger til en ven");
    m.insert(NAVIGATE, "Naviger");
    m.insert(OSM_MAP, "OSM-kort");
    m.insert(NO_LOCATIONS, "Ingen placeringer at navigere til.");
    m.insert(SEARCH_PLACES, "Søg efter steder (OpenStreetMap)");
    m.insert(SEARCH, "Søg");
    m.insert(RESULTS, "Resultater");
    m.insert(LANGUAGE, "Sprog");
    m.insert(PROFILE_VISIBILITY, "Profilens synlighed");
    m.insert(FRIENDS_ONLY, "Kun venner");
    m.insert(PUBLIC, "Offentlig");
    m.insert(SIGN_IN_REQUIRED, "Login påkrævet");
    m.insert(SIGN_IN_TO_USE, "For at bruge {feature}, log ind først.");
    m.insert(GO_TO_MAP, "Gå til kortet");
    m.insert(
        OFFLINE_MAP_HINT,
        "Eller bliv i korttilstand — den virker offline uden login.",
    );
    m
}

fn fi() -> HashMap<&'static str, &'static str> {
    use key::*;
    let mut m = HashMap::new();
    m.insert(MAP, "Kartta");
    m.insert(FRIENDS, "Kaverit");
    m.insert(SHARING, "Jakaminen");
    m.insert(PLACES, "Paikat");
    m.insert(SAVED, "Tallennetut");
    m.insert(TRIPS, "Matkat");
    m.insert(EMERGENCY, "Hätätilanne");
    m.insert(PROFILE, "Profiili");
    m.insert(ADMIN, "Ylläpito");
    m.insert(KIOSK, "Kiosk");
    m.insert(PASSWORD, "Salasana");
    m.insert(SIGN_IN, "Kirjaudu sisään");
    m.insert(SIGN_OUT, "Kirjaudu ulos");
    m.insert(OFFLINE_MODE, "Offline-tila");
    m.insert(REFRESH, "Päivitä");
    m.insert(FRIEND_LOCATIONS, "Kaverien sijainnit");
    m.insert(PUBLISH_MY_LOCATION, "Julkaise sijaintini");
    m.insert(LATITUDE, "Leveysaste");
    m.insert(LONGITUDE, "Pituusaste");
    m.insert(PUBLISH, "Julkaise");
    m.insert(NO_FRIENDS, "Ei vielä kavereita");
    m.insert(ACCEPT, "Hyväksy");
    m.insert(REJECT, "Hylkää");
    m.insert(AWAITING_RESPONSE, "odotetaan vastausta");
    m.insert(LOCATION_SHARING, "Sijainnin jakaminen");
    m.insert(NO_SHARING, "Ei jako-oikeuksia");
    m.insert(PAUSE, "Tauko");
    m.insert(RESUME, "Jatka");
    m.insert(NAVIGATE_TO_FRIEND, "Navigoi kaverille");
    m.insert(NAVIGATE, "Navigoi");
    m.insert(OSM_MAP, "OSM-kartta");
    m.insert(NO_LOCATIONS, "Ei sijainteja navigointiin.");
    m.insert(SEARCH_PLACES, "Etsi paikkoja (OpenStreetMap)");
    m.insert(SEARCH, "Etsi");
    m.insert(RESULTS, "Tulokset");
    m.insert(LANGUAGE, "Kieli");
    m.insert(PROFILE_VISIBILITY, "Profiilin näkyvyys");
    m.insert(FRIENDS_ONLY, "Vain kaverit");
    m.insert(PUBLIC, "Julkinen");
    m.insert(SIGN_IN_REQUIRED, "Kirjautuminen vaaditaan");
    m.insert(
        SIGN_IN_TO_USE,
        "Käyttääksesi {feature}, kirjaudu ensin sisään.",
    );
    m.insert(GO_TO_MAP, "Siirry karttaan");
    m.insert(
        OFFLINE_MAP_HINT,
        "Tai pysy karttatilassa — se toimii offline-tilassa ilman kirjautumista.",
    );
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_covers_every_language_and_codes_are_unique() {
        let langs = Lang::all();
        assert!(langs.len() >= 70, "registry must be broad");
        let mut codes: Vec<&str> = langs.iter().map(|l| l.code()).collect();
        let n = codes.len();
        codes.sort_unstable();
        codes.dedup();
        assert_eq!(codes.len(), n, "language codes must be unique");
    }

    #[test]
    fn fallback_to_english_for_untranslated_language() {
        // e.g. Swahili has no table -> English fallback.
        let s = t(Lang::Sw, key::SIGN_IN);
        assert_eq!(s, "Sign in");
    }

    #[test]
    fn every_language_block_has_all_keys() {
        let en = table().get(&Lang::En).unwrap();
        for lang in [
            Lang::Ro,
            Lang::Ru,
            Lang::Fr,
            Lang::De,
            Lang::Es,
            Lang::Pt,
            Lang::It,
            Lang::Nl,
            Lang::Pl,
            Lang::Uk,
            Lang::Tr,
            Lang::Ar,
            Lang::He,
            Lang::Zh,
            Lang::Ja,
            Lang::Ko,
            Lang::Hi,
            Lang::Vi,
            Lang::Th,
            Lang::Id,
            Lang::El,
            Lang::Cs,
            Lang::Hu,
            Lang::Sv,
            Lang::No,
            Lang::Da,
            Lang::Fi,
        ] {
            let m = table().get(&lang).expect("translation block");
            for k in en.keys() {
                assert!(m.contains_key(*k), "{lang:?} missing key {k}");
            }
        }
    }

    #[test]
    fn locale_parsing_handles_common_forms() {
        assert_eq!(Lang::from_locale("ro-MD"), Some(Lang::Ro));
        assert_eq!(Lang::from_locale("ru_RU"), Some(Lang::Ru));
        assert_eq!(Lang::from_locale("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::from_locale("de"), Some(Lang::De));
        assert_eq!(Lang::from_locale("xx"), None);
    }

    #[test]
    fn system_language_never_panics() {
        let _ = Lang::system();
    }

    #[test]
    fn english_table_is_complete() {
        let en = table().get(&Lang::En).unwrap();
        assert!(en.len() >= 40, "expected ~40+ keys, got {}", en.len());
    }
}
