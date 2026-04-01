/**
 * DidiCafe i18n — client-side localization
 *
 * Usage in HTML:
 *   <span data-i18n="portal.title">Enter your access token</span>
 *   <input data-i18n-placeholder="portal.placeholder" placeholder="DIDI-XXXX-XXXX">
 *   <button data-i18n-aria="portal.connect_aria" aria-label="Connect to internet">
 *
 * The switcher buttons use: <button class="lang-btn" data-lang="mg">MG</button>
 *
 * Language is persisted in localStorage under "didicafe_lang".
 * Default: Malagasy (mg) — primary audience.
 */

"use strict";

const TRANSLATIONS = {
  mg: {
    // -- Portal --
    "portal.brand": "DidiCafe",
    "portal.subtitle": "WiFi an'ny rehetra",
    "portal.title": "Ampidiro ny code-nao",
    "portal.placeholder": "DIDI-XXXX-XXXX",
    "portal.connect": "Hiditra",
    "portal.connect_aria": "Hiditra amin'ny internet",
    "portal.error.invalid": "Code diso na efa nampiasaina.",
    "portal.error.rate_limit": "Andraso kely azafady, efa be loatra ny fanandramana.",
    "portal.error.device": "Tsy hita ny fitaovanao. Hamarino fa mifandray amin'ny WiFi ianao.",
    "portal.error.internal": "Nisy olana. Andramo indray azafady.",
    "portal.error.csrf": "Fangatahana tsy mety. Andramo indray azafady.",
    "portal.error.expired": "Tapitra ny session. Mividia code vaovao azafady.",

    // -- Success --
    "success.title": "Tafiditra!",
    "success.remaining": "Fotoana sisa",
    "success.hint": "Azonao idiana ny pejy, manomboka mijery pejy hafa ianao.",
    "success.minutes": "min",

    // -- Expired --
    "expired.title": "Tapitra ny fotoana",
    "expired.message": "Tapitra ny fidirana amin'ny internet.",
    "expired.hint": "Mankanesa any amin'ny mpapiasa raha hividy code vaovao.",
    "expired.new_token": "Hampiditra code vaovao",

    // -- Admin Nav --
    "admin.nav.dashboard": "Dashboard",
    "admin.nav.tokens": "Code",
    "admin.nav.plans": "Drafitra",
    "admin.nav.sessions": "Lozisialy",
    "admin.nav.audit": "Journal",
    "admin.nav.logout": "Hivoaka",

    // -- Admin Login --
    "admin.login.title": "Fidirana ho an'ny mpiasa",
    "admin.login.username": "Anarana",
    "admin.login.password": "Teny miafina",
    "admin.login.submit": "Hiditra",
    "admin.login.error": "Anarana na teny miafina diso.",
    "admin.login.error.rate_limit": "Andraso kely azafady, efa be loatra ny fanandramana.",

    // -- Admin Dashboard --
    "admin.dash.title": "Dashboard",
    "admin.dash.active": "Mandeha",
    "admin.dash.sold_today": "Code namidy androany",
    "admin.dash.revenue": "Vola androany",
    "admin.dash.sessions": "Mandeha",
    "admin.dash.no_sessions": "Tsy misy lozisialy mandeha.",
    "admin.dash.col_mac": "MAC",
    "admin.dash.col_ip": "IP",
    "admin.dash.col_remaining": "Sisa",
    "admin.dash.disconnect": "Hajanona",
    "admin.dash.manage_tokens": "Fitantanana ny code",

    // -- Admin Tokens --
    "admin.tokens.title": "Fitantanana code",
    "admin.tokens.generate": "Mamorona code vaovao",
    "admin.tokens.select_plan": "Misafidiana drafitra...",
    "admin.tokens.name": "Anarana",
    "admin.tokens.quantity": "Isa",
    "admin.tokens.submit": "Mamorona",
    "admin.tokens.generated": "Code naorina",
    "admin.tokens.all": "Code rehetra",
    "admin.tokens.col_name": "Anarana",
    "admin.tokens.col_plan": "Drafitra",
    "admin.tokens.col_code": "Code",
    "admin.tokens.col_status": "Sata",
    "admin.tokens.col_created": "Naorina",
    "admin.tokens.col_expires": "Tapitra",
    "admin.tokens.back": "Hiverina",

    // -- Admin Plans --
    "admin.plans.title": "Fitantanana drafitra",
    "admin.plans.create": "Mamorona drafitra vaovao",
    "admin.plans.name": "Anarana",
    "admin.plans.duration": "Faharetan'ny (minitra)",
    "admin.plans.price": "Vidiny (Ariary)",
    "admin.plans.submit": "Mamorona",
    "admin.plans.all": "Drafitra rehetra",
    "admin.plans.col_name": "Anarana",
    "admin.plans.col_duration": "Minitra",
    "admin.plans.col_price": "Ariary",
    "admin.plans.col_status": "Sata",
    "admin.plans.active": "Mavitrika",
    "admin.plans.inactive": "Tsy mavitrika",
    "admin.plans.deactivate": "Hajanona",
    "admin.plans.activate": "Hamelona",
    "admin.plans.back": "Hiverina",

    // -- Admin Sessions --
    "admin.sessions.title": "Fitantanana lozisialy",
    "admin.sessions.none": "Tsy misy lozisialy mandeha.",
    "admin.sessions.count": "Isa:",
    "admin.sessions.col_mac": "MAC",
    "admin.sessions.col_ip": "IP",
    "admin.sessions.col_started": "Nanomboka",
    "admin.sessions.col_expires": "Tapitra",
    "admin.sessions.col_remaining": "Sisa",
    "admin.sessions.disconnect": "Hajanona",
    "admin.sessions.back": "Hiverina",

    // -- Admin Audit --
    "admin.audit.title": "Journal d'audit",
    "admin.audit.none": "Tsy misy fidirana.",
    "admin.audit.col_time": "Daty",
    "admin.audit.col_user": "Admin",
    "admin.audit.col_action": "Hetsika",
    "admin.audit.col_target": "Cible",
    "admin.audit.col_detail": "Antsipirihany",
    "admin.audit.action_login": "Fidirana",
    "admin.audit.action_create_plan": "Drafitra noforonina",
    "admin.audit.action_generate_tokens": "Code namboarina",
    "admin.audit.action_disconnect": "Fanapahana",
    "admin.audit.action_revoke": "Fanafoanana",

    // -- Admin Manage --
    "admin.manage.title": "Fitantanana",
    "admin.manage.plan_created": "Drafitra '{{name}}' noforonina.",
    "admin.manage.tokens_generated": "Code {{count}} naorina.",
    "admin.plans.edit": "Fahasahana drafitra",
    "admin.plans.cancel": "Aoka",
    "admin.plans.save": "Tehirizo",

    // -- Admin Errors (validation) --
    "admin.error.csrf": "Fangatahana tsy mety. Andramo indray azafady.",
    "admin.error.duration_required": "Ny faharetan'ny ilaina.",
    "admin.error.price_required": "Ny vidiny ilaina.",
    "admin.error.name_too_long_200": "Ny anarana dia tsy maintsy latsaky ny 200 litera.",
    "admin.error.name_empty": "Ny anarana tsy maintsy feno.",
    "admin.error.name_too_long": "Ny anarana dia tsy maintsy latsaky ny 100 litera.",
    "admin.error.duration_positive": "Ny faharetan'ny dia tsy maintsy > 0.",
    "admin.error.duration_max": "Ny faharetan'ny dia tsy maintsy <= 1440 minitra (24h).",
    "admin.error.price_negative": "Ny vidiny dia tsy maintsy >= 0.",

    // -- Generic Errors --
    "error.internal": "Nisy olana. Andramo indray azafady.",
    "error.rate_limit": "Andraso kely azafady, efa be loatra ny fanandramana.",
    "error.constraint": "Fanoroana tsy mety na efa misy.",

    // -- Status --
    "status.active": "mandeha",
    "status.unused": "mbola tsy nampiasaina",
    "status.expired": "tapitra",
    "status.revoked": "nesorina",

    // -- Polling notifications --
    "notify.activated": "Code %s lasa mandeha",
    "notify.expired": "Code %s tapitra",

    // -- Privacy --
    "privacy.title": "Mombamomba ny tsiambaratelo",
    "privacy.intro": "DidiCafe dia manangona angon-drakitra ilaina fotsiny ho an'ny fampiasana WiFi.",
    "privacy.data_title": "Angon-drakitra angonina",
    "privacy.data_mac": "Adiresy MAC — famantarana tokana ny fitaovanao, ampiasaina hanomezana alalana hiditra amin'ny tambajotra. Voafafa mandeha hoazy rehefa tapitra ny session.",
    "privacy.data_session": "Faharetan'ny session — ora nanombohana sy hifarana, ho an'ny fotoana novidina.",
    "privacy.data_not_title": "Angon-drakitra TSY angonina",
    "privacy.data_not_browsing": "Tantaran'ny navigasion — tsy hitantsika na voatahirizy ny tranonkala alehanao.",
    "privacy.data_not_content": "Votoatin'ny fifandraisana — tsy vakina na voatahirizy ny hafatrao, mailaka ary rakitra.",
    "privacy.data_not_identity": "Mombamomba anao — tsy mangataka anarana, laharana finday na taratasy maha-olona.",
    "privacy.retention_title": "Fitehirizana",
    "privacy.retention": "Ny angon-drakitra session dia voafafa mandeha hoazy rehefa afaka 90 andro. Ny code tapitra na nesorina dia voafafa koa.",
    "privacy.rights_title": "Zo anao",
    "privacy.rights": "Afaka mangataka ny fanafoanana ny angonao amin'ny mpiasan'ny cafe.",
    "privacy.security_title": "Fiarovana",
    "privacy.security": "Voatahirizy eto an-toerana ny angon-drakitra. Tsy alefa any amin'ny antoko hafa na any amin'ny internet.",
    "privacy.back": "Hiverina",
    "portal.privacy": "Mifandray, ny MAC-nao sy IP-nao dia voatahirizy mandritra ny session-nao.",
    "portal.privacy_link": "Hahalala bebe kokoa",
  },

  fr: {
    // -- Portal --
    "portal.brand": "DidiCafe",
    "portal.subtitle": "WiFi pour tous",
    "portal.title": "Entrez votre code d'accès",
    "portal.placeholder": "DIDI-XXXX-XXXX",
    "portal.connect": "Se connecter",
    "portal.connect_aria": "Se connecter à internet",
    "portal.error.invalid": "Code invalide ou déjà utilisé.",
    "portal.error.rate_limit": "Trop de tentatives. Veuillez patienter.",
    "portal.error.device": "Appareil non identifié. Vérifiez votre connexion WiFi.",
    "portal.error.internal": "Erreur interne. Veuillez réessayer.",
    "portal.error.csrf": "Requête invalide. Veuillez réessayer.",
    "portal.error.expired": "Session expirée. Veuillez acheter un nouveau code.",

    // -- Success --
    "success.title": "Connecté !",
    "success.remaining": "Temps restant",
    "success.hint": "Vous pouvez fermer cette page et naviguer librement.",
    "success.minutes": "min",

    // -- Expired --
    "expired.title": "Session expirée",
    "expired.message": "Votre temps d'accès internet est terminé.",
    "expired.hint": "Rendez-vous à l'accueil pour acheter un nouveau code.",
    "expired.new_token": "Entrer un nouveau code",

    // -- Admin Nav --
    "admin.nav.dashboard": "Tableau de bord",
    "admin.nav.tokens": "Codes",
    "admin.nav.plans": "Forfaits",
    "admin.nav.sessions": "Sessions",
    "admin.nav.logout": "Déconnexion",

    // -- Admin Login --
    "admin.login.title": "Connexion personnel",
    "admin.login.username": "Nom d'utilisateur",
    "admin.login.password": "Mot de passe",
    "admin.login.submit": "Se connecter",
    "admin.login.error": "Identifiants invalides.",
    "admin.login.error.rate_limit": "Trop de tentatives. Veuillez patienter.",

    // -- Admin Dashboard --
    "admin.dash.title": "Tableau de bord",
    "admin.dash.active": "Sessions actives",
    "admin.dash.sold_today": "Codes vendus",
    "admin.dash.revenue": "Revenu du jour",
    "admin.dash.sessions": "Sessions actives",
    "admin.dash.no_sessions": "Aucune session active.",
    "admin.dash.col_mac": "MAC",
    "admin.dash.col_ip": "IP",
    "admin.dash.col_remaining": "Restant",
    "admin.dash.disconnect": "Déconnecter",
    "admin.dash.manage_tokens": "Gérer les codes",

    // -- Admin Tokens --
    "admin.tokens.title": "Gestion des codes",
    "admin.tokens.generate": "Générer de nouveaux codes",
    "admin.tokens.select_plan": "Choisir un forfait...",
    "admin.tokens.name": "Nom",
    "admin.tokens.quantity": "Quantité",
    "admin.tokens.submit": "Générer",
    "admin.tokens.generated": "Codes générés",
    "admin.tokens.all": "Codes courants",
    "admin.tokens.col_name": "Nom",
    "admin.tokens.col_plan": "Forfait",
    "admin.tokens.col_code": "Code",
    "admin.tokens.col_status": "Statut",
    "admin.tokens.col_created": "Créé le",
    "admin.tokens.col_expires": "Expire le",
    "admin.tokens.back": "Retour",

    // -- Admin Plans --
    "admin.plans.title": "Gestion des forfaits",
    "admin.plans.create": "Créer un nouveau forfait",
    "admin.plans.name": "Nom",
    "admin.plans.duration": "Durée (minutes)",
    "admin.plans.price": "Prix (Ariary)",
    "admin.plans.submit": "Créer",
    "admin.plans.all": "Forfaits",
    "admin.plans.col_name": "Nom",
    "admin.plans.col_duration": "Minutes",
    "admin.plans.col_price": "Ariary",
    "admin.plans.col_status": "Statut",
    "admin.plans.active": "Actif",
    "admin.plans.inactive": "Inactif",
    "admin.plans.deactivate": "Désactiver",
    "admin.plans.activate": "Activer",
    "admin.plans.back": "Retour",

    // -- Admin Sessions --
    "admin.sessions.title": "Gestion des sessions",
    "admin.sessions.none": "Aucune session active.",
    "admin.sessions.count": "Nombre:",
    "admin.sessions.col_mac": "MAC",
    "admin.sessions.col_ip": "IP",
    "admin.sessions.col_started": "Démarré",
    "admin.sessions.col_expires": "Expire",
    "admin.sessions.col_remaining": "Restant",
    "admin.sessions.disconnect": "Déconnecter",
    "admin.sessions.back": "Retour",

    // -- Admin Audit --
    "admin.audit.title": "Journal d'audit",
    "admin.audit.none": "Aucune entrée.",
    "admin.audit.col_time": "Date",
    "admin.audit.col_user": "Admin",
    "admin.audit.col_action": "Action",
    "admin.audit.col_target": "Cible",
    "admin.audit.col_detail": "Détail",
    "admin.audit.action_login": "Connexion",
    "admin.audit.action_create_plan": "Forfait créé",
    "admin.audit.action_generate_tokens": "Codes générés",
    "admin.audit.action_disconnect": "Déconnexion",
    "admin.audit.action_revoke": "Révocation",

    // -- Admin Manage --
    "admin.manage.title": "Gestion",
    "admin.manage.plan_created": "Forfait '{{name}}' créé.",
    "admin.manage.tokens_generated": "{{count}} code(s) généré(s).",
    "admin.plans.edit": "Modifier le forfait",
    "admin.plans.cancel": "Annuler",
    "admin.plans.save": "Enregistrer",

    // -- Admin Errors (validation) --
    "admin.error.csrf": "Requête invalide. Veuillez réessayer.",
    "admin.error.duration_required": "La durée est requise.",
    "admin.error.price_required": "Le prix est requis.",
    "admin.error.name_too_long_200": "Le nom doit faire 200 caractères maximum.",
    "admin.error.name_empty": "Le nom ne doit pas être vide.",
    "admin.error.name_too_long": "Le nom doit faire 100 caractères maximum.",
    "admin.error.duration_positive": "La durée doit être supérieure à 0.",
    "admin.error.duration_max": "La durée ne doit pas dépasser 1440 minutes (24h).",
    "admin.error.price_negative": "Le prix doit être supérieur ou égal à 0.",

    // -- Generic Errors --
    "error.internal": "Erreur interne. Veuillez réessayer.",
    "error.rate_limit": "Trop de tentatives. Veuillez patienter.",
    "error.constraint": "Référence invalide ou entrée en doublon.",

    // -- Status --
    "status.active": "actif",
    "status.unused": "non utilisé",
    "status.expired": "expiré",
    "status.revoked": "révoqué",

    // -- Polling notifications --
    "notify.activated": "Code %s activé",
    "notify.expired": "Code %s expiré",

    // -- Privacy --
    "privacy.title": "Confidentialité",
    "privacy.intro": "DidiCafe ne collecte que les données strictement nécessaires au fonctionnement du WiFi.",
    "privacy.data_title": "Données collectées",
    "privacy.data_mac": "Adresse MAC — identifiant de votre appareil, utilisé uniquement pour autoriser l'accès réseau. Supprimée automatiquement après expiration.",
    "privacy.data_session": "Durée de session — heure de début et d'expiration, pour gérer le temps acheté.",
    "privacy.data_not_title": "Données NON collectées",
    "privacy.data_not_browsing": "Historique de navigation — nous ne voyons ni ne stockons les sites visités.",
    "privacy.data_not_content": "Contenu de vos communications — messages, emails et fichiers ne sont ni lus ni stockés.",
    "privacy.data_not_identity": "Identité personnelle — aucun nom, numéro de téléphone ou pièce d'identité n'est demandé.",
    "privacy.retention_title": "Conservation",
    "privacy.retention": "Les données de session sont supprimées automatiquement après 90 jours.",
    "privacy.rights_title": "Vos droits",
    "privacy.rights": "Vous pouvez demander la suppression de vos données auprès du personnel du café.",
    "privacy.security_title": "Sécurité",
    "privacy.security": "Les données sont stockées localement. Aucune transmission à des tiers ou à internet.",
    "privacy.back": "Retour",
    "portal.privacy": "En vous connectant, votre MAC et IP sont stockés pour la durée de votre session.",
    "portal.privacy_link": "En savoir plus",
  },

  en: {
    // -- Portal --
    "portal.brand": "DidiCafe",
    "portal.subtitle": "WiFi for everyone",
    "portal.title": "Enter your access code",
    "portal.placeholder": "DIDI-XXXX-XXXX",
    "portal.connect": "Connect",
    "portal.connect_aria": "Connect to internet",
    "portal.error.invalid": "Invalid or already used code.",
    "portal.error.rate_limit": "Too many attempts. Please wait.",
    "portal.error.device": "Device not identified. Check your WiFi connection.",
    "portal.error.internal": "Internal error. Please try again.",
    "portal.error.csrf": "Invalid request. Please try again.",
    "portal.error.expired": "Session expired. Please purchase a new code.",

    // -- Success --
    "success.title": "Connected!",
    "success.remaining": "Time remaining",
    "success.hint": "You can close this page and start browsing.",
    "success.minutes": "min",

    // -- Expired --
    "expired.title": "Session Expired",
    "expired.message": "Your internet access time has ended.",
    "expired.hint": "Please visit the front desk for a new access code.",
    "expired.new_token": "Enter New Code",

    // -- Admin Nav --
    "admin.nav.dashboard": "Dashboard",
    "admin.nav.tokens": "Tokens",
    "admin.nav.plans": "Plans",
    "admin.nav.sessions": "Sessions",
    "admin.nav.logout": "Log out",

    // -- Admin Login --
    "admin.login.title": "Staff Login",
    "admin.login.username": "Username",
    "admin.login.password": "Password",
    "admin.login.submit": "Log in",
    "admin.login.error": "Invalid credentials.",
    "admin.login.error.rate_limit": "Too many attempts. Please wait and try again.",

    // -- Admin Dashboard --
    "admin.dash.title": "Dashboard",
    "admin.dash.active": "Active Sessions",
    "admin.dash.sold_today": "Tokens Sold Today",
    "admin.dash.revenue": "Revenue Today",
    "admin.dash.sessions": "Active Sessions",
    "admin.dash.no_sessions": "No active sessions.",
    "admin.dash.col_mac": "MAC",
    "admin.dash.col_ip": "IP",
    "admin.dash.col_remaining": "Remaining",
    "admin.dash.disconnect": "Disconnect",
    "admin.dash.manage_tokens": "Manage Tokens",

    // -- Admin Tokens --
    "admin.tokens.title": "Token Management",
    "admin.tokens.generate": "Generate New Tokens",
    "admin.tokens.select_plan": "Select a plan...",
    "admin.tokens.name": "Name",
    "admin.tokens.quantity": "Quantity",
    "admin.tokens.submit": "Generate",
    "admin.tokens.generated": "Generated tokens",
    "admin.tokens.all": "All Tokens",
    "admin.tokens.col_name": "Name",
    "admin.tokens.col_plan": "Plan",
    "admin.tokens.col_code": "Code",
    "admin.tokens.col_status": "Status",
    "admin.tokens.col_created": "Created",
    "admin.tokens.col_expires": "Expires",
    "admin.tokens.back": "Back",

    // -- Admin Plans --
    "admin.plans.title": "Plan Management",
    "admin.plans.create": "Create New Plan",
    "admin.plans.name": "Name",
    "admin.plans.duration": "Duration (minutes)",
    "admin.plans.price": "Price (Ariary)",
    "admin.plans.submit": "Create",
    "admin.plans.all": "All Plans",
    "admin.plans.col_name": "Name",
    "admin.plans.col_duration": "Minutes",
    "admin.plans.col_price": "Ariary",
    "admin.plans.col_status": "Status",
    "admin.plans.active": "Active",
    "admin.plans.inactive": "Inactive",
    "admin.plans.deactivate": "Deactivate",
    "admin.plans.activate": "Activate",
    "admin.plans.back": "Back",

    // -- Admin Sessions --
    "admin.sessions.title": "Session Management",
    "admin.sessions.none": "No active sessions.",
    "admin.sessions.count": "Count:",
    "admin.sessions.col_mac": "MAC",
    "admin.sessions.col_ip": "IP",
    "admin.sessions.col_started": "Started",
    "admin.sessions.col_expires": "Expires",
    "admin.sessions.col_remaining": "Remaining",
    "admin.sessions.disconnect": "Disconnect",
    "admin.sessions.back": "Back",

    // -- Admin Audit --
    "admin.audit.title": "Audit log",
    "admin.audit.none": "No entries.",
    "admin.audit.col_time": "Time",
    "admin.audit.col_user": "Admin",
    "admin.audit.col_action": "Action",
    "admin.audit.col_target": "Target",
    "admin.audit.col_detail": "Detail",
    "admin.audit.action_login": "Login",
    "admin.audit.action_create_plan": "Plan created",
    "admin.audit.action_generate_tokens": "Tokens generated",
    "admin.audit.action_disconnect": "Disconnect",
    "admin.audit.action_revoke": "Revocation",

    // -- Admin Manage --
    "admin.manage.title": "Management",
    "admin.manage.plan_created": "Plan '{{name}}' created.",
    "admin.manage.tokens_generated": "{{count}} token(s) generated.",
    "admin.plans.edit": "Edit plan",
    "admin.plans.cancel": "Cancel",
    "admin.plans.save": "Save",

    // -- Admin Errors (validation) --
    "admin.error.csrf": "Invalid request. Please try again.",
    "admin.error.duration_required": "Duration is required.",
    "admin.error.price_required": "Price is required.",
    "admin.error.name_too_long_200": "Name must be 200 characters or less.",
    "admin.error.name_empty": "Name must not be empty.",
    "admin.error.name_too_long": "Name must be 100 characters or less.",
    "admin.error.duration_positive": "Duration must be greater than 0.",
    "admin.error.duration_max": "Duration must not exceed 1440 minutes (24h).",
    "admin.error.price_negative": "Price must be 0 or greater.",

    // -- Generic Errors --
    "error.internal": "Internal error. Please try again.",
    "error.rate_limit": "Too many attempts. Please wait and try again.",
    "error.constraint": "Invalid reference or duplicate entry.",

    // -- Status --
    "status.active": "active",
    "status.unused": "unused",
    "status.expired": "expired",
    "status.revoked": "revoked",

    // -- Polling notifications --
    "notify.activated": "Code %s activated",
    "notify.expired": "Code %s expired",

    // -- Privacy --
    "privacy.title": "Privacy",
    "privacy.intro": "DidiCafe only collects data strictly necessary for WiFi service operation.",
    "privacy.data_title": "Data collected",
    "privacy.data_mac": "MAC address — unique device identifier, used only to authorize network access. Automatically deleted after session expiry.",
    "privacy.data_session": "Session duration — start and expiry time, to manage purchased access time.",
    "privacy.data_not_title": "Data NOT collected",
    "privacy.data_not_browsing": "Browsing history — we do not see or store the sites you visit.",
    "privacy.data_not_content": "Communication content — your messages, emails and files are neither read nor stored.",
    "privacy.data_not_identity": "Personal identity — no name, phone number or ID is requested.",
    "privacy.retention_title": "Retention",
    "privacy.retention": "Session data is automatically deleted after 90 days.",
    "privacy.rights_title": "Your rights",
    "privacy.rights": "You may request deletion of your data by contacting cafe staff.",
    "privacy.security_title": "Security",
    "privacy.security": "Data is stored locally. No data is transmitted to third parties or the internet.",
    "privacy.back": "Back",
    "portal.privacy": "By connecting, your MAC and IP are stored for the duration of your session.",
    "portal.privacy_link": "Learn more",
  },
};

const DEFAULT_LANG = "mg";
const STORAGE_KEY = "didicafe_lang";

/** Get current language from localStorage or default. */
function getLang() {
  try {
    return localStorage.getItem(STORAGE_KEY) || DEFAULT_LANG;
  } catch {
    return DEFAULT_LANG;
  }
}

/** Set language, persist, and update the page. */
function setLang(lang) {
  if (!TRANSLATIONS[lang]) return;
  try {
    localStorage.setItem(STORAGE_KEY, lang);
  } catch { /* ignore */ }
  applyLang(lang);
}

/**
 * Translate a single key with optional interpolation.
 *
 * @param {string} key   - Translation key (e.g. "admin.manage.plan_created")
 * @param {string} [lang] - Language code; defaults to current language
 * @param {Object} [args] - Named interpolation values, e.g. { count: "5", name: "WiFi 1h" }
 * @returns {string} Translated string with {{placeholders}} replaced, or the key itself if not found
 *
 * Example:
 *   Translation: "{{count}} code(s) créé(s)"
 *   t("admin.manage.tokens_generated", "fr", { count: "5" })
 *   => "5 code(s) créé(s)"
 */
function t(key, lang, args) {
  lang = lang || getLang();
  var val = (TRANSLATIONS[lang] && TRANSLATIONS[lang][key]) || key;
  if (args) {
    var keys = Object.keys(args);
    for (var i = 0; i < keys.length; i++) {
      val = val.replace(new RegExp("\\{\\{" + keys[i] + "\\}\\}", "g"), args[keys[i]]);
    }
  }
  return val;
}

/** Apply translations to all data-i18n elements on the page. */
function applyLang(lang) {
  lang = lang || getLang();

  // Update html lang attribute
  document.documentElement.lang = lang === "mg" ? "mg" : lang === "fr" ? "fr" : "en";

  // Text content (with optional interpolation via data-i18n-args)
  document.querySelectorAll("[data-i18n]").forEach(function (el) {
    var key = el.getAttribute("data-i18n");
    var args = null;
    var argsAttr = el.getAttribute("data-i18n-args");
    if (argsAttr) {
      try { args = JSON.parse(argsAttr); } catch (_) { /* ignore malformed JSON */ }
    }
    var val = t(key, lang, args);
    if (val !== key) el.textContent = val;
  });

  // Placeholders
  document.querySelectorAll("[data-i18n-placeholder]").forEach(function (el) {
    var key = el.getAttribute("data-i18n-placeholder");
    var val = t(key, lang);
    if (val !== key) el.placeholder = val;
  });

  // Aria labels
  document.querySelectorAll("[data-i18n-aria]").forEach(function (el) {
    var key = el.getAttribute("data-i18n-aria");
    var val = t(key, lang);
    if (val !== key) el.setAttribute("aria-label", val);
  });

  // Update switcher active state
  document.querySelectorAll(".lang-btn").forEach(function (btn) {
    btn.classList.toggle("active", btn.getAttribute("data-lang") === lang);
  });
}

/** Initialize: bind switcher buttons and apply current language. */
function initI18n() {
  document.querySelectorAll(".lang-btn").forEach(function (btn) {
    btn.addEventListener("click", function () {
      setLang(btn.getAttribute("data-lang"));
    });
  });
  applyLang(getLang());
}

// Auto-init on DOMContentLoaded
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initI18n);
} else {
  initI18n();
}