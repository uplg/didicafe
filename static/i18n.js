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
    "admin.nav.logout": "Hivoaka",

    // -- Admin Login --
    "admin.login.title": "Fidirana ho an'ny mpiasa",
    "admin.login.username": "Anarana",
    "admin.login.password": "Teny miafina",
    "admin.login.submit": "Hiditra",
    "admin.login.error": "Anarana na teny miafina diso.",

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

    // -- Admin Manage --
    "admin.manage.title": "Fitantanana",
    "admin.plans.edit": "Fahasahana drafitra",
    "admin.plans.cancel": "Aoka",
    "admin.plans.save": "Tehirizo",

    // -- Status --
    "status.active": "mandeha",
    "status.unused": "mbola tsy nampiasaina",
    "status.expired": "tapitra",
    "status.revoked": "nesorina",

    // -- Privacy --
    "privacy.title": "Mombamomba ny tsiambaratelo",
    "privacy.intro": "DidiCafe dia manangona angon-drakitra ilaina fotsiny ho an'ny fampiasana WiFi.",
    "privacy.data_title": "Angon-drakitra angonina",
    "privacy.data_mac": "Adiresy MAC — famantarana tokana ny fitaovanao, voatahirizy ho an'ny fampiditra amin'ny tambajotra.",
    "privacy.data_ip": "Adiresy IP — nomen'ny DHCP, ampiasaina ho an'ny fanaraha-maso session.",
    "privacy.data_session": "Faharetan'ny session — ora nanombohana sy hifarana, ho an'ny fotoana novidina.",
    "privacy.retention_title": "Fitehirizana",
    "privacy.retention": "Ny angon-drakitra session dia voafafa mandeha hoazy rehefa afaka 90 andro.",
    "privacy.rights_title": "Zo anao",
    "privacy.rights": "Afaka mangataka ny fanafoanana ny angonao amin'ny mpiasan'ny cafe.",
    "privacy.security_title": "Fiarovana",
    "privacy.security": "Voatahirizy eto an-toerana ny angon-drakitra. Tsy alefa any amin'ny antoko hafa.",
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

    // -- Admin Manage --
    "admin.manage.title": "Gestion",
    "admin.plans.edit": "Modifier le forfait",
    "admin.plans.cancel": "Annuler",
    "admin.plans.save": "Enregistrer",

    // -- Status --
    "status.active": "actif",
    "status.unused": "non utilisé",
    "status.expired": "expiré",
    "status.revoked": "révoqué",

    // -- Privacy --
    "privacy.title": "Confidentialité",
    "privacy.intro": "DidiCafe ne collecte que les données nécessaires au fonctionnement du WiFi.",
    "privacy.data_title": "Données collectées",
    "privacy.data_mac": "Adresse MAC — identifiant de votre appareil, stockée pour gérer l'accès réseau.",
    "privacy.data_ip": "Adresse IP — assignée par DHCP, utilisée pour le suivi de session.",
    "privacy.data_session": "Durée de session — heure de début et d'expiration, pour gérer le temps acheté.",
    "privacy.retention_title": "Conservation",
    "privacy.retention": "Les données de session sont supprimées automatiquement après 90 jours.",
    "privacy.rights_title": "Vos droits",
    "privacy.rights": "Vous pouvez demander la suppression de vos données auprès du personnel du café.",
    "privacy.security_title": "Sécurité",
    "privacy.security": "Les données sont stockées localement. Aucune transmission à des tiers.",
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

    // -- Admin Manage --
    "admin.manage.title": "Management",
    "admin.plans.edit": "Edit plan",
    "admin.plans.cancel": "Cancel",
    "admin.plans.save": "Save",

    // -- Status --
    "status.active": "active",
    "status.unused": "unused",
    "status.expired": "expired",
    "status.revoked": "revoked",

    // -- Privacy --
    "privacy.title": "Privacy",
    "privacy.intro": "DidiCafe only collects data necessary for WiFi service operation.",
    "privacy.data_title": "Data collected",
    "privacy.data_mac": "MAC address — unique device identifier, stored for network access management.",
    "privacy.data_ip": "IP address — assigned by DHCP, used for session tracking.",
    "privacy.data_session": "Session duration — start and expiry time, to manage purchased access time.",
    "privacy.retention_title": "Retention",
    "privacy.retention": "Session data is automatically deleted after 90 days.",
    "privacy.rights_title": "Your rights",
    "privacy.rights": "You may request deletion of your data by contacting cafe staff.",
    "privacy.security_title": "Security",
    "privacy.security": "Data is stored locally. No data is transmitted to third parties.",
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

/** Translate a single key. Returns the key itself if not found. */
function t(key, lang) {
  lang = lang || getLang();
  return (TRANSLATIONS[lang] && TRANSLATIONS[lang][key]) || key;
}

/** Apply translations to all data-i18n elements on the page. */
function applyLang(lang) {
  lang = lang || getLang();

  // Update html lang attribute
  document.documentElement.lang = lang === "mg" ? "mg" : lang === "fr" ? "fr" : "en";

  // Text content
  document.querySelectorAll("[data-i18n]").forEach(function (el) {
    var key = el.getAttribute("data-i18n");
    var val = t(key, lang);
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