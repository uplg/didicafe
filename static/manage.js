"use strict";

/**
 * Manage page plan operations.
 *
 * Handles plan toggling (active/inactive), plan editing via the modal dialog,
 * and plan saving via the REST API.
 */

function togglePlan(id, activate, btn) {
    btn.disabled = true;
    fetch("/api/plans/" + id + "/active", {
        method: "PATCH",
        credentials: "same-origin",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ active: activate })
    }).then(function(r) { if (r.ok) window.location.reload(); });
}

function editPlanFromEl(el) {
    editPlan(
        el.getAttribute("data-plan-id"),
        el.getAttribute("data-plan-name"),
        el.getAttribute("data-plan-duration"),
        el.getAttribute("data-plan-price")
    );
}

function editPlan(id, name, duration, price) {
    document.getElementById("edit-id").value = id;
    document.getElementById("edit-name").value = name;
    document.getElementById("edit-duration").value = duration;
    document.getElementById("edit-price").value = price;
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
    }).then(function(r) { 
        if (r.ok) {
            closeEditModal();
            window.location.reload();
        }
    });
}
