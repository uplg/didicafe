"use strict";

/**
 * Success page countdown timer with SVG progress ring.
 *
 * Reads the initial remaining seconds from a data attribute on #countdown,
 * then counts down every second. Syncs with the server every 60s via
 * /portal/status.json to correct clock drift.
 *
 * Adaptive display format:
 *   - >= 60 min: "1h 23min"
 *   - < 60 min:  "23:45"
 *
 * Drives the SVG progress ring stroke-dashoffset using data-total.
 */
(function () {
    var el = document.getElementById("countdown");
    if (!el) return;

    var remaining = parseInt(el.getAttribute("data-remaining"), 10) || 0;
    var total = parseInt(el.getAttribute("data-total"), 10) || remaining;
    if (remaining <= 0) return;

    var ring = document.getElementById("progress-ring");
    // Circumference = 2 * PI * r (r=52 from the SVG)
    var circumference = 2 * Math.PI * 52; // ~326.73

    function pad(n) { return n < 10 ? "0" + n : "" + n; }

    /**
     * Adaptive time format:
     *   >= 3600s (60min): "1h 23min"
     *   < 3600s:          "23:45"
     */
    function formatTime(secs) {
        if (secs <= 0) return "0:00";

        var h = Math.floor(secs / 3600);
        var m = Math.floor((secs % 3600) / 60);
        var s = secs % 60;

        if (h > 0) {
            return h + "h " + pad(m) + "min";
        }
        return m + ":" + pad(s);
    }

    /** Update the SVG progress ring based on remaining/total ratio. */
    function updateRing() {
        if (!ring) return;
        var fraction = total > 0 ? Math.max(0, remaining / total) : 0;
        var offset = circumference * (1 - fraction);
        ring.style.strokeDashoffset = offset;
    }

    function render() {
        if (remaining <= 0) {
            el.textContent = "0:00";
            updateRing();
            window.location.href = "/portal/expired";
            return;
        }
        el.textContent = formatTime(remaining);
        updateRing();
        remaining--;
    }

    // Sync with server every 60s to correct drift
    function sync() {
        fetch("/portal/status.json")
            .then(function (r) { return r.json(); })
            .then(function (data) {
                if (!data.connected) {
                    window.location.href = "/portal/expired";
                    return;
                }
                remaining = data.remaining_seconds;
            })
            .catch(function () { /* ignore -- client countdown continues */ });
    }

    render();
    setInterval(render, 1000);
    setInterval(sync, 60000);
})();
