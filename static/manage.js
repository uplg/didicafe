"use strict";

/**
 * Manage page operations (CSP-compliant — no inline handlers).
 *
 * Uses event delegation on the document body. Actions are identified
 * by data attributes:
 *   - data-toggle-plan="<id>" data-activate="true|false" — toggle plan active state
 *   - data-edit-plan="<id>" — open edit modal for a plan
 *   - data-copy-code="<code>" — copy a single token code to clipboard
 *   - data-action="close-modal" — close the edit modal
 *
 * The edit form (#edit-plan-form) uses a submit event listener.
 */
document.addEventListener("DOMContentLoaded", function () {
    // Event delegation for all button actions
    document.body.addEventListener("click", function (e) {
        var btn;

        // Toggle plan active/inactive
        btn = e.target.closest("[data-toggle-plan]");
        if (btn) {
            togglePlan(btn);
            return;
        }

        // Edit plan — open modal
        btn = e.target.closest("[data-edit-plan]");
        if (btn) {
            editPlanFromEl(btn);
            return;
        }

        // Copy single token code
        btn = e.target.closest("[data-copy-code]");
        if (btn) {
            copyCode(btn);
            return;
        }

        // Close edit modal
        btn = e.target.closest("[data-action='close-modal']");
        if (btn) {
            closeEditModal();
            return;
        }
    });

    // Edit plan form submission
    var editForm = document.getElementById("edit-plan-form");
    if (editForm) {
        editForm.addEventListener("submit", function (e) {
            e.preventDefault();
            saveEditPlan();
        });
    }
});

function togglePlan(btn) {
    var id = btn.getAttribute("data-toggle-plan");
    var activate = btn.getAttribute("data-activate") === "true";
    btn.disabled = true;
    fetch("/api/plans/" + id + "/active", {
        method: "PATCH",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ active: activate })
    }).then(function (r) { if (r.ok) window.location.reload(); });
}

function editPlanFromEl(el) {
    document.getElementById("edit-id").value = el.getAttribute("data-edit-plan");
    document.getElementById("edit-name").value = el.getAttribute("data-plan-name");
    document.getElementById("edit-duration").value = el.getAttribute("data-plan-duration");
    document.getElementById("edit-price").value = el.getAttribute("data-plan-price");
    document.getElementById("edit-modal").showModal();
}

function closeEditModal() {
    document.getElementById("edit-modal").close();
}

function saveEditPlan() {
    var id = document.getElementById("edit-id").value;
    var name = document.getElementById("edit-name").value;
    var duration = parseInt(document.getElementById("edit-duration").value) || 60;
    var price = parseInt(document.getElementById("edit-price").value) || 0;

    fetch("/api/plans/" + id, {
        method: "PUT",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
            name: name,
            duration_minutes: duration,
            price_ariary: price,
            active: true
        })
    }).then(function (r) {
        if (r.ok) {
            closeEditModal();
            window.location.reload();
        }
    });
}

/**
 * Copy a single token code to clipboard.
 * Uses navigator.clipboard with fallback to execCommand for HTTP contexts.
 * Shows a brief checkmark feedback on the button.
 */
function copyCode(btn) {
    var code = btn.getAttribute("data-copy-code");
    if (!code) return;

    if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(code).then(function () {
            showCopyFeedback(btn);
        }).catch(function () {
            fallbackCopy(code, btn);
        });
    } else {
        fallbackCopy(code, btn);
    }
}

/** Fallback copy using a temporary textarea (for HTTP contexts). */
function fallbackCopy(text, btn) {
    var ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    try {
        document.execCommand("copy");
        showCopyFeedback(btn);
    } catch (_) { /* ignore */ }
    document.body.removeChild(ta);
}

/** Brief visual feedback: swap icon to checkmark for 1.5s. */
function showCopyFeedback(btn) {
    if (!btn) return;
    var original = btn.innerHTML;
    btn.innerHTML = '<svg width="14" height="14" viewBox="0 0 16 16" fill="none" stroke="var(--c-success)" stroke-width="2" stroke-linecap="round"><path d="M3 8l4 4 6-8"/></svg>';
    btn.disabled = true;
    setTimeout(function () {
        btn.innerHTML = original;
        btn.disabled = false;
    }, 1500);
}
