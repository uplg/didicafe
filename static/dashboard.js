"use strict";

/**
 * Dashboard polling script.
 *
 * Polls /api/sessions and /api/stats every 10 seconds, rebuilds the
 * sessions table, updates stat counters, and shows toast notifications
 * when sessions are activated or expire.
 */

var POLL_INTERVAL = 10000;
var knownSessions = {};

// Seed known sessions from the server-rendered page
(function() {
    var rows = document.querySelectorAll("[id^='session-']");
    rows.forEach(function(row) {
        var id = row.id.replace("session-", "");
        var code = row.querySelector("code");
        knownSessions[id] = code ? code.textContent.trim() : id;
    });
})();

function showToast(message, type) {
    var container = document.getElementById("toast-container");
    if (!container) return;
    var el = document.createElement("div");
    el.className = "toast toast--" + type;
    el.textContent = message;
    container.appendChild(el);
    // Trigger animation
    requestAnimationFrame(function() { el.classList.add("show"); });
    setTimeout(function() {
        el.classList.remove("show");
        setTimeout(function() { el.remove(); }, 300);
    }, 5000);
}

function formatNotify(key, code) {
    return t(key).replace("%s", code);
}

function rebuildSessionsTable(sessions) {
    var area = document.getElementById("sessions-area");
    if (!area) return;
    if (sessions.length === 0) {
        area.innerHTML = '<p class="text-muted text-sm mt-sm" data-i18n="admin.dash.no_sessions">' +
            t("admin.dash.no_sessions") + '</p>';
        return;
    }
    var html = '<div class="table-wrap"><table><thead><tr>' +
        '<th>Code</th>' +
        '<th data-i18n="admin.dash.col_mac">' + t("admin.dash.col_mac") + '</th>' +
        '<th data-i18n="admin.dash.col_ip">' + t("admin.dash.col_ip") + '</th>' +
        '<th data-i18n="admin.dash.col_remaining">' + t("admin.dash.col_remaining") + '</th>' +
        '<th><span class="sr-only">Actions</span></th>' +
        '</tr></thead><tbody>';
    sessions.forEach(function(s) {
        var code = s.token_code || "";
        var name = s.token_name || "";
        var remaining = Math.max(0, Math.floor((Date.parse(s.expires_at.replace(" ", "T") + "Z") - Date.now()) / 60000));
        html += '<tr id="session-' + s.id + '">' +
            '<td><code>' + escHtml(code) + '</code> <span class="text-muted text-xs">' + escHtml(name) + '</span></td>' +
            '<td><code>' + escHtml(s.mac_address) + '</code></td>' +
            '<td>' + escHtml(s.ip_address) + '</td>' +
            '<td>' + remaining + ' min</td>' +
            '<td><button type="button" class="btn btn--danger btn--sm" data-i18n="admin.dash.disconnect" onclick="disconnectSession(' + s.id + ', this)" aria-label="Disconnect session ' + s.id + '">' + t("admin.dash.disconnect") + '</button></td>' +
            '</tr>';
    });
    html += '</tbody></table></div>';
    area.innerHTML = html;
}

function escHtml(s) {
    var d = document.createElement("div");
    d.appendChild(document.createTextNode(s));
    return d.innerHTML;
}

function poll() {
    Promise.all([
        fetch("/api/sessions", { credentials: "same-origin" }).then(function(r) { return r.ok ? r.json() : null; }),
        fetch("/api/stats", { credentials: "same-origin" }).then(function(r) { return r.ok ? r.json() : null; })
    ]).then(function(results) {
        var sessData = results[0];
        var statsData = results[1];

        if (sessData && sessData.data && sessData.data.sessions) {
            var sessions = sessData.data.sessions;
            var newMap = {};
            sessions.forEach(function(s) {
                newMap[s.id] = s.token_code || String(s.id);
            });

            // Detect new sessions (activated tokens)
            Object.keys(newMap).forEach(function(id) {
                if (!knownSessions[id]) {
                    showToast(formatNotify("notify.activated", newMap[id]), "activated");
                }
            });

            // Detect removed sessions (expired/disconnected tokens)
            Object.keys(knownSessions).forEach(function(id) {
                if (!newMap[id]) {
                    showToast(formatNotify("notify.expired", knownSessions[id]), "expired");
                }
            });

            knownSessions = newMap;
            rebuildSessionsTable(sessions);
        }

        if (statsData && statsData.data) {
            var s = statsData.data;
            var el;
            el = document.getElementById("stat-active");
            if (el) el.textContent = s.active_sessions;
            el = document.getElementById("stat-sold");
            if (el) el.textContent = s.tokens_sold;
            el = document.getElementById("stat-revenue");
            if (el) el.textContent = s.revenue_ariary + " Ar";
        }
    }).catch(function() { /* network error, retry next tick */ });
}

setInterval(poll, POLL_INTERVAL);
