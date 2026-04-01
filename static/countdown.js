"use strict";

/**
 * Success page countdown timer.
 *
 * Reads the initial remaining seconds from a data attribute on #countdown,
 * then counts down every second. Syncs with the server every 60s via
 * /portal/status to correct clock drift.
 */
(function() {
    var el = document.getElementById("countdown");
    if (!el) return;

    var totalSeconds = parseInt(el.getAttribute("data-remaining"), 10) || 0;
    if (totalSeconds <= 0) return;

    function pad(n) { return n < 10 ? "0" + n : "" + n; }

    function render() {
        if (totalSeconds <= 0) {
            el.textContent = "0:00";
            window.location.href = "/portal/expired";
            return;
        }
        var m = Math.floor(totalSeconds / 60);
        var s = totalSeconds % 60;
        el.textContent = m + ":" + pad(s);
        totalSeconds--;
    }

    // Sync with server every 60s to correct drift
    function sync() {
        fetch("/portal/status")
            .then(function(r) { return r.json(); })
            .then(function(data) {
                if (!data.connected) {
                    window.location.href = "/portal/expired";
                    return;
                }
                totalSeconds = data.remaining_seconds;
            })
            .catch(function() { /* ignore -- client countdown continues */ });
    }

    render();
    setInterval(render, 1000);
    setInterval(sync, 60000);
})();
