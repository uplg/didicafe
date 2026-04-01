# PLAN-refine.md — Raffinement UX + Architecture

> Statut global : **EN ATTENTE VALIDATION**
> Dernière MàJ : 2026-04-01

---

## Vue d'ensemble

4 chantiers, ordonnés par dépendance :

| # | Chantier | Priorité | Effort |
|---|----------|----------|--------|
| A | Dual-listener HTTP/HTTPS + domaine local | P0 | Moyen |
| B | Sécurisation admin (port séparé, pas Ethernet-only) | P0 | Faible |
| C | Page publique "Forfaits & Contact" | P1 | Moyen |
| D | Refonte UX/UI polish | P1 | Élevé |

---

## A — Dual-listener HTTP portal + HTTPS admin

### Problème actuel

Le portail tourne sur `http://10.10.0.1:8080`. L'utilisateur voit une IP brute.
L'admin transite en clair sur le réseau WiFi partagé (login, cookies, tokens).

### Contraintes

- Le portail captif **DOIT** rester en HTTP : les mécanismes CPD (CaptiveAgent iOS,
  NetworkMonitor Android, NCSI Windows, NetworkManager Linux) ne suivent pas les
  redirections HTTPS et cassent sur les certificats self-signed.
- Le gérant doit pouvoir administrer depuis son **téléphone sur le même WiFi** —
  pas de contrainte Ethernet-only ni de réseau staff séparé.

### Architecture cible

| Listener | Port | Proto | Domaine | Contenu | Accès |
|---|---|---|---|---|---|
| Portal | 8080 | HTTP | `didicafe.local` | `/portal/*`, `/static/*`, CPD, fallback | Tous (DNAT nftables) |
| Admin | 8443 | HTTPS (rustls) | `didicafe.local:8443` | `/admin/*`, `/api/*`, `/static/*` | Ceux qui connaissent le port |

- **dnsmasq** résout `didicafe.local` → `10.10.0.1` (config dnsmasq, hors code Rust)
- Le domaine est configurable : `[portal] domain = "didicafe.local"`
- Le DNAT nftables redirige **uniquement** le port 80 → `:8080`. Le port `8443`
  n'est pas DNATé — il faut le connaître explicitement.
- Ce n'est pas de la sécurité par obscurité comme défense principale : rate limiter
  + Argon2id restent la vraie barrière. Le port séparé réduit la surface d'attaque.

### Détail d'implémentation

1. **Config TOML — nouvelles sections**

   ```toml
   [portal]
   domain = "didicafe.local"

   [tls]
   enabled = true
   cert_path = "/opt/didicafe/certs/server.pem"
   key_path = "/opt/didicafe/certs/server-key.pem"
   admin_port = 8443
   ```

   - `tls.enabled = false` en dev (pas besoin de certs pour `cargo run`)
   - Quand TLS est désactivé, l'admin est servi sur le listener HTTP principal
     (comportement actuel, pour le développement local)

2. **Deux `tokio::spawn` dans `main.rs`**
   - Listener HTTP (portal) : `TcpListener::bind("0.0.0.0:8080")`
   - Listener HTTPS (admin) : `TlsListener` via `tokio-rustls` sur `:8443`
   - Chacun a son propre `Router` mais partage le même `Arc<AppState>`
   - Le router portal n'a PAS les routes `/admin/*` ni `/api/*`
   - Le router admin n'a PAS les routes `/portal/*` ni CPD

3. **Certificat self-signed avec CA racine locale**
   - Script `scripts/gen-certs.sh` génère via `openssl` :
     - `certs/ca.pem` + `certs/ca-key.pem` — CA racine (10 ans)
     - `certs/server.pem` + `certs/server-key.pem` — cert serveur (2 ans)
     - SAN : `DNS:didicafe.local`, `IP:10.10.0.1`
   - Le gérant installe `ca.pem` sur son téléphone/PC une seule fois
   - Instructions d'installation dans le README

4. **Redirections CPD**
   - Pointent vers `http://didicafe.local/portal` (HTTP, pas HTTPS)
   - Le domaine configurable est utilisé dans les 302 au lieu du chemin relatif

5. **Cookie `Secure` flag**
   - Activé **uniquement** sur les cookies admin (servis en HTTPS)
   - Les cookies portal restent sans `Secure` (HTTP obligatoire)

6. **nftables**
   - Port 8443 ouvert en input sur toutes les interfaces (WiFi + Ethernet)
   - Le DNAT ne touche PAS au port 8443
   ```
   # Admin HTTPS - accessible directement (pas de DNAT)
   tcp dport 8443 accept
   ```

### Fichiers impactés
- `src/config.rs` — `PortalConfig`, `TlsConfig`, validation
- `src/main.rs` — Dual listener setup, router split
- `src/web/mod.rs` — `portal_router()` et `admin_router()` séparés, CPD domain
- `src/web/cpd.rs` — Redirections vers `http://{domain}/portal`
- `src/web/admin.rs` — Cookie `Secure` flag conditionnel
- `config/didicafe.toml` — Sections `[portal]` et `[tls]`
- `config/didicafe.dev.toml` — `tls.enabled = false`
- `config/nftables.conf` — Port 8443 en input accept
- `scripts/gen-certs.sh` — NOUVEAU
- `Cargo.toml` — `tokio-rustls`, `rustls-pemfile`

---

## B — Sécurisation admin sur réseau unique

### Problème actuel

Admin et portal partagent le même listener. Un client WiFi peut accéder à `/admin`.

### Solution (intégrée dans A)

La séparation en deux listeners (chantier A) résout le problème :

1. **Le port 8443 n'est pas DNATé** — un client non-auth dont tout le trafic est
   redirigé vers `:8080` ne peut pas atteindre `:8443`
2. **Un client auth** (qui a payé un token) PEUT techniquement atteindre `:8443`
   car son trafic n'est plus intercepté. C'est acceptable car :
   - Rate limiter bloque le bruteforce (5 tentatives / 60s, ban après 10)
   - Argon2id rend le bruteforce computationnellement impossible
   - HTTPS chiffre le trafic admin (pas de sniffing de credentials)
   - Le port non-standard réduit la surface de scan automatique

3. **Defense-in-depth optionnel** (config, pas implémenté en phase 1) :
   - `admin.allowed_networks` dans le TOML — si défini, le middleware vérifie
     l'IP source. Si non défini, toutes les IPs sont acceptées.
   - Permet au gérant de restreindre plus tard s'il ajoute un réseau staff

### Fichiers impactés
- Pas de fichiers supplémentaires — tout est couvert par le chantier A
- `src/config.rs` — `admin.allowed_networks` optionnel (Vec vide = pas de restriction)

---

## C — Page publique "Forfaits & Contact"

### Problème actuel

Aucune page ne liste les forfaits disponibles. Un voisin qui veut acheter un accès
quand le café est fermé n'a aucun moyen de contacter le gérant.

### Solution

1. **Nouvelle route** : `GET /portal/plans` — page publique (router portal, HTTP)
   - Affiche les plans **actifs** (nom, durée, prix en Ariary) en cards
   - PAS de données sensibles (pas de tokens, pas de sessions, pas de stats)
   - Accessible sans authentification, avant même d'avoir un token

2. **Section contact configurable**
   - Affichée sur la page plans ET en footer de la page portail
   - Config TOML :
     ```toml
     [portal]
     domain = "didicafe.local"
     cafe_name = "DidiCafe"
     contact_phone = "+261 34 XX XXX XX"    # optionnel
     contact_name = "Didi"                   # optionnel
     contact_hours = "Lun-Sam 7h-20h"       # optionnel
     ```
   - Lien `tel:` cliquable sur mobile
   - Lien WhatsApp `https://wa.me/261XXXXXXXX` si numéro mobile détecté

3. **Navigation portail**
   - Lien "Forfaits" en bas de la page portail (à côté du lien privacy)
   - La page plans a un bouton retour vers le portail

4. **Template** : `templates/plans.html` (extends `base.html`)
   - Grid responsive de cards forfaits
   - Chaque card : nom, durée formatée (ex: "1 ora" / "30 minitra"), prix
   - Section contact en bas

5. **i18n** : Nouvelles clés dans `i18n.js` (mg/fr/en)

### Fichiers impactés
- `src/config.rs` — `PortalConfig` étendu (contact fields)
- `src/web/portal.rs` — `plans_page()` handler, `PlansTemplate`
- `src/db/mod.rs` — `list_active_plans()` (query publique, sans données sensibles)
- `templates/plans.html` — NOUVEAU
- `templates/portal.html` — Lien vers /portal/plans, section contact
- `static/i18n.js` — Nouvelles clés plans/contact
- `static/style.css` — Styles cards forfaits
- `config/didicafe.toml` — Contact info dans `[portal]`

---

## D — Refonte UX/UI polish

### Philosophie

L'UI actuelle est propre et fonctionnelle. L'objectif est d'atteindre un **degré de
finition premium** : micro-interactions, transitions, hiérarchie visuelle, feedback
utilisateur, et personnalisation — sans ajouter de dépendances externes.

### D.1 — Design system upgrade

| Élément | Actuel | Cible |
|---------|--------|-------|
| Logo | Lettre "D" dans un carré amber | SVG inline (lettre D avec motif café) configurable via `cafe_name` |
| Palette | Warm amber fixe | Configurable via `[portal] theme_color` → CSS custom properties injectées côté serveur |
| Typographie | System fonts | Même stack, meilleur scale typographique + spacing |
| Dark mode | Absent | `prefers-color-scheme: dark` automatique, palette sombre calculée |
| Animations | Pulse ring sur success | Micro-transitions cohérentes partout |

### D.2 — Portal UX polish

1. **Page portail (token input)**
   - **Auto-formatage** du code pendant la frappe : insère les tirets automatiquement
   - **Loading state** sur le bouton à la soumission (spinner CSS pur)
   - **Shake animation** sur le card en cas d'erreur
   - Aperçu discret des forfaits sous le formulaire (prix min → max) + lien "Voir les forfaits"
   - Message d'accueil configurable : `[portal] welcome_message` (optionnel)

2. **Page success (connecté)**
   - **Progress ring SVG** circulaire qui se vide progressivement au lieu du ring statique
   - Affichage du **nom du plan** actif (ex: "WiFi 1h")
   - Countdown adaptatif : `1h 23min` quand > 60min, `23:45` quand < 60min

3. **Page expired**
   - CTA plus visible pour acheter un nouveau code
   - Contact rapide du gérant

### D.3 — Admin UX polish

1. **Navigation**
   - Indicateur de page active (highlight background)
   - Le nom du café configurable apparaît dans la nav

2. **Dashboard**
   - Icônes SVG inline dans les stat cards (WiFi, ticket, Ariary)
   - Transitions fluides au polling (count-up animation sur les chiffres)

3. **Manage page**
   - `<details>` → sections avec boutons toggle plus visibles
   - Bloc copiable avec bouton "Copier tout" pour les tokens générés
   - Badge compteurs dans les headers de section

4. **Toasts**
   - Unifiés et utilisés partout (admin + portal)
   - Slide-in depuis le haut, auto-dismiss 4s

### D.4 — Personnalisation configurable

```toml
[portal]
domain = "didicafe.local"
cafe_name = "DidiCafe"               # Nom affiché partout
welcome_message = ""                  # Message custom optionnel
theme_color = "#b45309"               # Couleur primaire (hex)
contact_phone = "+261 34 XX XXX XX"
contact_name = "Didi"
contact_hours = "Lun-Sam 7h-20h"
```

Le serveur injecte un `<style>` en tête de `base.html` avec les custom properties
calculées (hover = darken 15%, light = lighten 90%, etc.) à partir de `theme_color`.

### Fichiers impactés (D)
- `static/style.css` — Dark mode, animations, progress ring, cards forfaits
- `static/portal.js` — NOUVEAU : auto-format token, loading state, shake
- `static/countdown.js` — Format adaptatif, progress ring SVG
- `static/dashboard.js` — Count-up animation, transitions
- `static/manage.js` — UX sections toggle, copy button
- `static/i18n.js` — Nouvelles clés
- `templates/base.html` — Injection theme color, dark mode
- `templates/portal.html` — Aperçu forfaits, contact, animations
- `templates/success.html` — Progress ring SVG, plan name
- `templates/expired.html` — CTA amélioré, contact
- `templates/admin/base.html` — Active page indicator, cafe_name
- `templates/admin/dashboard.html` — Icônes SVG
- `templates/admin/manage.html` — Sections toggle, copy button
- `src/config.rs` — `PortalConfig` complet
- `src/web/portal.rs` — Injection config dans templates
- `src/web/mod.rs` — Injection CSS custom properties

---

## Ordre d'implémentation

```
Phase 1 : Infrastructure (A + B)
  ├── A.1 Config [portal] et [tls] + validation
  ├── A.2 Router split (portal_router / admin_router)
  ├── A.3 TLS listener via tokio-rustls
  ├── A.4 Dual spawn (HTTP + HTTPS)
  ├── A.5 CPD redirections vers http://{domain}/portal
  ├── A.6 Cookie Secure sur admin uniquement
  ├── A.7 Script gen-certs.sh
  ├── A.8 nftables port 8443
  ├── B.1 admin.allowed_networks optionnel
  └── A/B.9 Tests + clippy + build

Phase 2 : Fonctionnalités (C)
  ├── C.1 PortalConfig contact fields
  ├── C.2 Handler plans_page + PlansTemplate
  ├── C.3 Template plans.html + styles
  ├── C.4 Liens navigation portal
  ├── C.5 i18n (mg/fr/en)
  └── C.6 Tests + clippy + build

Phase 3 : UX Polish (D) — itératif
  ├── D.1 Design tokens configurables + dark mode
  ├── D.2 Portal polish (auto-format, loading, shake, progress ring)
  ├── D.3 Admin polish (nav active, icons, copy button, sections)
  ├── D.4 Page plans styling
  └── D.5 Final pass (toasts, transitions, responsive check)
```

Chaque phase se termine par : `cargo clippy && cargo test && cargo build --release`
→ 0 warnings, tous tests passent.

---

## Contraintes maintenues

- Zero `cargo clippy` warnings
- Zero `#[allow(unused)]` sans justification
- Tous les tests passent
- `thiserror` pour les erreurs domaine, `anyhow` uniquement dans `main`
- Pas de CDN, pas de framework JS, pas de build step
- Fonctionne offline (avant auth) — tous les assets servis localement
- OWASP best practices
- Malagasy (mg) comme langue par défaut
- Compatible BPI-R3 Mini, Alpine Linux, ~130 MB
- Réseau unique (pas de VLAN staff/public)
