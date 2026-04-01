"use strict";

/**
 * Disconnect an active session via the REST API.
 *
 * Shared between dashboard.html and manage.html to avoid code duplication.
 * On success, removes the corresponding table row from the DOM.
 *
 * @param {number} id - Session ID to disconnect.
 * @param {HTMLButtonElement} btn - The button that triggered the action (disabled during request).
 */
function disconnectSession(id, btn) {
    btn.disabled = true;
    var originalText = btn.textContent;
    btn.textContent = "...";
    fetch("/api/sessions/" + id, { method: "DELETE", credentials: "same-origin" })
        .then(function(res) {
            if (res.ok) {
                var row = document.getElementById("session-" + id);
                if (row) row.remove();
            } else {
                btn.disabled = false;
                btn.textContent = "Error";
                setTimeout(function() {
                    btn.textContent = typeof t === "function" ? t("admin.dash.disconnect") : originalText;
                }, 2000);
            }
        })
        .catch(function() {
            btn.disabled = false;
            btn.textContent = "Error";
        });
}
