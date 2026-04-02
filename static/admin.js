"use strict";

/**
 * Shared admin functionality: disconnect session via REST API.
 *
 * Uses event delegation on the document body. Any button with
 * `data-disconnect="<session_id>"` triggers the disconnect flow.
 * This avoids inline onclick handlers (CSP-compliant).
 */
document.addEventListener("DOMContentLoaded", function () {
    document.body.addEventListener("click", function (e) {
        var btn = e.target.closest("[data-disconnect]");
        if (!btn) return;

        var id = btn.getAttribute("data-disconnect");
        btn.disabled = true;
        var originalText = btn.textContent;
        btn.textContent = "...";

        fetch("/api/sessions/" + id, { method: "DELETE", credentials: "same-origin" })
            .then(function (res) {
                if (res.ok) {
                    var row = document.getElementById("session-" + id);
                    if (row) row.remove();
                } else {
                    btn.disabled = false;
                    btn.textContent = t("error.generic");
                    setTimeout(function () {
                        btn.textContent = typeof t === "function" ? t("admin.dash.disconnect") : originalText;
                    }, 2000);
                }
            })
            .catch(function () {
                btn.disabled = false;
                btn.textContent = t("error.generic");
            });
    });
});
