"use strict";

/**
 * Settings page: sync color picker with text input.
 */
document.addEventListener("DOMContentLoaded", function () {
    var picker = document.getElementById("theme_color_picker");
    var text = document.getElementById("theme_color");
    if (!picker || !text) return;

    picker.addEventListener("input", function () {
        text.value = picker.value;
    });

    text.addEventListener("input", function () {
        var val = text.value.trim();
        if (/^#[0-9a-fA-F]{6}$/.test(val)) {
            picker.value = val;
        }
    });
});
