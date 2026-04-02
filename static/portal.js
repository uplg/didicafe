"use strict";

/**
 * Portal page enhancements.
 *
 * - Auto-formats token input as DIDI-XXXX-XXXX while typing
 * - Adds loading state (btn--loading class) on form submit
 * - Triggers shake animation on card when error is present
 */
(function () {
    var input = document.getElementById("token");
    var form = document.getElementById("portal-form");
    var btn = document.getElementById("portal-submit");

    if (!input) return;

    /**
     * Auto-format token input: DIDI-XXXX-XXXX
     *
     * Strips non-alphanumeric chars, uppercases, then inserts dashes
     * at positions 4 and 8 to match the DIDI-XXXX-XXXX pattern.
     */
    input.addEventListener("input", function () {
        var raw = input.value.replace(/[^A-Za-z0-9]/g, "").toUpperCase();
        if (raw.length > 12) raw = raw.slice(0, 12);

        var formatted = "";
        for (var i = 0; i < raw.length; i++) {
            if (i === 4 || i === 8) formatted += "-";
            formatted += raw[i];
        }
        input.value = formatted;
    });

    // Prevent paste from leaving unformatted text
    input.addEventListener("paste", function () {
        // Defer to after paste completes
        setTimeout(function () {
            input.dispatchEvent(new Event("input"));
        }, 0);
    });

    /**
     * Loading state on submit.
     *
     * Adds btn--loading class and disables button to prevent double-submit.
     * The class shows a spinner via CSS.
     */
    if (form && btn) {
        form.addEventListener("submit", function () {
            btn.classList.add("btn--loading");
            btn.disabled = true;
        });
    }

    /**
     * Shake animation cleanup.
     *
     * The server adds .shake to the card on error. We remove it after
     * the animation ends so it can re-trigger on next page load.
     */
    var card = document.querySelector(".card.shake");
    if (card) {
        card.addEventListener("animationend", function () {
            card.classList.remove("shake");
        });
    }
})();
