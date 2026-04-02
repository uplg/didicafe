"use strict";

/**
 * Dashboard polling + 7-day SVG chart (CSP-compliant).
 *
 * Features:
 *   - Polls /api/sessions, /api/stats, /api/stats/weekly every 10s
 *   - Rebuilds the sessions table on change
 *   - Count-up animation on stat cards
 *   - Toast notifications for activated/expired sessions
 *   - SVG line chart: revenue (area) + tokens sold (line) over 7 days
 *
 * Disconnect buttons use data-disconnect="<id>", handled by admin.js.
 */

var POLL_INTERVAL = 10000;
var COUNTUP_DURATION = 600;
var knownSessions = {};

// ── Seed known sessions ──────────────────────────────────────────────

(function() {
    var rows = document.querySelectorAll("[id^='session-']");
    rows.forEach(function(row) {
        var id = row.id.replace("session-", "");
        var code = row.querySelector("code");
        knownSessions[id] = code ? code.textContent.trim() : id;
    });
})();

// ── Count-up animation ──────────────────────────────────────────────

function animateValue(el, from, to, suffix) {
    if (from === to) return;
    var start = null;
    el.classList.add("updating");
    function step(ts) {
        if (!start) start = ts;
        var p = Math.min((ts - start) / COUNTUP_DURATION, 1);
        var eased = 1 - Math.pow(1 - p, 3);
        el.textContent = Math.round(from + (to - from) * eased) + suffix;
        if (p < 1) {
            requestAnimationFrame(step);
        } else {
            el.textContent = to + suffix;
            el.dataset.value = to;
            el.classList.remove("updating");
        }
    }
    requestAnimationFrame(step);
}

function updateStat(id, newValue) {
    var el = document.getElementById(id);
    if (!el) return;
    var oldValue = parseInt(el.dataset.value, 10) || 0;
    var suffix = el.dataset.suffix || "";
    if (oldValue !== newValue) {
        animateValue(el, oldValue, newValue, suffix);
    }
}

// ── Toasts ──────────────────────────────────────────────────────────

function showToast(message, type) {
    var container = document.getElementById("toast-container");
    if (!container) return;
    var el = document.createElement("div");
    el.className = "toast toast--" + type;
    el.textContent = message;
    container.appendChild(el);
    requestAnimationFrame(function() { el.classList.add("show"); });
    setTimeout(function() {
        el.classList.remove("show");
        setTimeout(function() { el.remove(); }, 300);
    }, 4000);
}

function formatNotify(key, code) {
    return t(key).replace("%s", code);
}

// ── Sessions table ──────────────────────────────────────────────────

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
            '<td><button type="button" class="btn btn--danger btn--sm" data-i18n="admin.dash.disconnect" data-disconnect="' + s.id + '" aria-label="Disconnect session ' + s.id + '">' + t("admin.dash.disconnect") + '</button></td>' +
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

// ── 7-Day SVG Chart ─────────────────────────────────────────────────

/**
 * Renders a dual-series SVG chart into #weekly-chart.
 *
 * Series 1 (revenue): area fill + line in brand color (left Y axis)
 * Series 2 (tokens):  line + dots in green (right Y axis)
 *
 * The chart uses a viewBox coordinate system for resolution independence.
 * Day labels shown on X axis, Y grid lines for revenue scale.
 *
 * @param {Array<{date:string, tokens_sold:number, revenue_ariary:number}>} days
 */
function renderChart(days) {
    var wrap = document.getElementById("weekly-chart");
    if (!wrap || !days || days.length === 0) return;

    // Chart dimensions (viewBox units)
    var W = 560, H = 200;
    var PAD_L = 55, PAD_R = 15, PAD_T = 10, PAD_B = 28;
    var CW = W - PAD_L - PAD_R;
    var CH = H - PAD_T - PAD_B;

    // Extract data
    var revenues = days.map(function(d) { return d.revenue_ariary; });
    var tokens = days.map(function(d) { return d.tokens_sold; });

    var maxRev = Math.max.apply(null, revenues);
    var maxTok = Math.max.apply(null, tokens);
    // Ensure non-zero scales; add 10% headroom
    if (maxRev === 0) maxRev = 1000;
    else maxRev = Math.ceil(maxRev * 1.1);
    if (maxTok === 0) maxTok = 5;
    else maxTok = Math.ceil(maxTok * 1.1);

    // Compute nice Y grid lines for revenue (4 lines)
    var gridCount = 4;
    var gridStep = niceStep(maxRev, gridCount);
    maxRev = gridStep * gridCount;

    function xPos(i) { return PAD_L + (i / (days.length - 1)) * CW; }
    function yRev(v) { return PAD_T + CH - (v / maxRev) * CH; }
    function yTok(v) { return PAD_T + CH - (v / maxTok) * CH; }

    // Build SVG string
    var svg = '<svg viewBox="0 0 ' + W + ' ' + H + '" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="7-day chart">';

    // Defs: gradient for revenue area
    svg += '<defs>';
    svg += '<linearGradient id="revGrad" x1="0" y1="0" x2="0" y2="1">';
    svg += '<stop offset="0%" stop-color="var(--c-primary)" stop-opacity="0.25"/>';
    svg += '<stop offset="100%" stop-color="var(--c-primary)" stop-opacity="0.02"/>';
    svg += '</linearGradient>';
    svg += '</defs>';

    // Y grid lines + labels (revenue)
    for (var g = 0; g <= gridCount; g++) {
        var gy = PAD_T + CH - (g / gridCount) * CH;
        var gv = gridStep * g;
        if (g > 0) {
            svg += '<line x1="' + PAD_L + '" y1="' + gy + '" x2="' + (W - PAD_R) + '" y2="' + gy + '" stroke="var(--c-border)" stroke-width="0.5" stroke-dasharray="3,3"/>';
        }
        svg += '<text x="' + (PAD_L - 6) + '" y="' + (gy + 3) + '" text-anchor="end" fill="var(--c-text-muted)" font-size="8" font-weight="500">' + formatK(gv) + '</text>';
    }

    // X axis baseline
    svg += '<line x1="' + PAD_L + '" y1="' + (PAD_T + CH) + '" x2="' + (W - PAD_R) + '" y2="' + (PAD_T + CH) + '" stroke="var(--c-border)" stroke-width="0.5"/>';

    // Revenue area fill
    var areaPath = 'M ' + xPos(0) + ' ' + yRev(revenues[0]);
    for (var i = 1; i < days.length; i++) {
        areaPath += ' L ' + xPos(i) + ' ' + yRev(revenues[i]);
    }
    areaPath += ' L ' + xPos(days.length - 1) + ' ' + (PAD_T + CH);
    areaPath += ' L ' + xPos(0) + ' ' + (PAD_T + CH) + ' Z';
    svg += '<path d="' + areaPath + '" fill="url(#revGrad)"/>';

    // Revenue line
    var revLine = 'M ' + xPos(0) + ' ' + yRev(revenues[0]);
    for (var j = 1; j < days.length; j++) {
        revLine += ' L ' + xPos(j) + ' ' + yRev(revenues[j]);
    }
    svg += '<path d="' + revLine + '" fill="none" stroke="var(--c-primary)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>';

    // Tokens line
    var tokLine = 'M ' + xPos(0) + ' ' + yTok(tokens[0]);
    for (var k = 1; k < days.length; k++) {
        tokLine += ' L ' + xPos(k) + ' ' + yTok(tokens[k]);
    }
    svg += '<path d="' + tokLine + '" fill="none" stroke="var(--c-success)" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/>';

    // Data points (dots) + X labels + invisible hover targets
    for (var m = 0; m < days.length; m++) {
        var x = xPos(m);

        // Revenue dot
        svg += '<circle cx="' + x + '" cy="' + yRev(revenues[m]) + '" r="3" fill="var(--c-primary)" stroke="var(--c-surface)" stroke-width="1.5"/>';

        // Token dot
        svg += '<circle cx="' + x + '" cy="' + yTok(tokens[m]) + '" r="3" fill="var(--c-success)" stroke="var(--c-surface)" stroke-width="1.5"/>';

        // Day label (short weekday or MM/DD)
        var label = formatDayLabel(days[m].date);
        svg += '<text x="' + x + '" y="' + (H - 6) + '" text-anchor="middle" fill="var(--c-text-muted)" font-size="8" font-weight="500">' + escHtml(label) + '</text>';

        // Invisible hover rect for tooltip
        var rectW = CW / days.length;
        svg += '<rect x="' + (x - rectW / 2) + '" y="' + PAD_T + '" width="' + rectW + '" height="' + CH + '" fill="transparent" data-chart-idx="' + m + '"/>';
    }

    svg += '</svg>';

    // Tooltip div
    svg += '<div class="chart-tooltip" id="chart-tooltip"></div>';

    wrap.innerHTML = svg;

    // Attach hover events
    setupChartTooltip(wrap, days, revenues, tokens, xPos);
}

/**
 * Compute a "nice" step value for grid lines.
 */
function niceStep(max, count) {
    var raw = max / count;
    var mag = Math.pow(10, Math.floor(Math.log10(raw)));
    var norm = raw / mag;
    var nice;
    if (norm <= 1) nice = 1;
    else if (norm <= 2) nice = 2;
    else if (norm <= 5) nice = 5;
    else nice = 10;
    return nice * mag;
}

/**
 * Format large numbers: 10000 → "10k", 1500 → "1.5k", 500 → "500"
 */
function formatK(v) {
    if (v >= 10000) return Math.round(v / 1000) + "k";
    if (v >= 1000) return (v / 1000).toFixed(v % 1000 === 0 ? 0 : 1) + "k";
    return String(v);
}

/**
 * Format date "YYYY-MM-DD" → short day label. Uses short weekday if
 * the Intl API is available, otherwise falls back to "DD/MM".
 */
function formatDayLabel(dateStr) {
    try {
        var parts = dateStr.split("-");
        var d = new Date(parseInt(parts[0], 10), parseInt(parts[1], 10) - 1, parseInt(parts[2], 10));
        // Short weekday in Malagasy locale (falls back to browser default)
        var wd = d.toLocaleDateString("mg", { weekday: "short" });
        if (wd && wd !== dateStr) return wd;
    } catch (e) { /* fallback */ }
    // Fallback: DD/MM
    return dateStr.slice(8) + "/" + dateStr.slice(5, 7);
}

/**
 * Setup tooltip interactivity on chart hover rects.
 */
function setupChartTooltip(wrap, days, revenues, tokens, xPos) {
    var tooltip = document.getElementById("chart-tooltip");
    if (!tooltip) return;

    var rects = wrap.querySelectorAll("[data-chart-idx]");
    rects.forEach(function(rect) {
        rect.addEventListener("mouseenter", function() {
            var idx = parseInt(rect.getAttribute("data-chart-idx"), 10);
            var day = days[idx];
            tooltip.innerHTML =
                '<div class="chart-tooltip-date">' + escHtml(day.date) + '</div>' +
                '<div class="chart-tooltip-row"><span class="chart-tooltip-dot" style="background:var(--c-primary)"></span>' + day.revenue_ariary + ' Ar</div>' +
                '<div class="chart-tooltip-row"><span class="chart-tooltip-dot" style="background:var(--c-success)"></span>' + day.tokens_sold + ' codes</div>';

            // Position tooltip above the chart column
            var svgEl = wrap.querySelector("svg");
            var wrapRect = wrap.getBoundingClientRect();
            var svgRect = svgEl.getBoundingClientRect();
            var scaleX = svgRect.width / 560;
            var leftPx = svgRect.left - wrapRect.left + xPos(idx) * scaleX;
            tooltip.style.left = leftPx + "px";
            tooltip.style.top = "10px";
            tooltip.classList.add("visible");
        });

        rect.addEventListener("mouseleave", function() {
            tooltip.classList.remove("visible");
        });
    });
}

// ── Chart loading ───────────────────────────────────────────────────

function loadChart() {
    fetch("/api/stats/weekly", { credentials: "same-origin" })
        .then(function(r) { return r.ok ? r.json() : null; })
        .then(function(data) {
            if (data && data.data && data.data.days) {
                renderChart(data.data.days);
            }
        })
        .catch(function() { /* ignore — chart stays in loading state */ });
}

// Load chart on page load
loadChart();

// ── Polling ─────────────────────────────────────────────────────────

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

            Object.keys(newMap).forEach(function(id) {
                if (!knownSessions[id]) {
                    showToast(formatNotify("notify.activated", newMap[id]), "activated");
                }
            });

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
            updateStat("stat-active", s.active_sessions);
            updateStat("stat-sold", s.tokens_sold);
            updateStat("stat-revenue", s.revenue_ariary);
        }
    }).catch(function() { /* network error, retry next tick */ });
}

setInterval(poll, POLL_INTERVAL);

// Refresh chart every 60s (less frequent than stat polling)
setInterval(loadChart, 60000);
