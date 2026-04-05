// Virtual Keyboard overlay for mobile WASM
// Shows a native input field above the canvas when Rust sets a "wants input" flag.
// The overlay covers the full screen to block touch events on the canvas.
// Focus happens in a touchend handler (iOS requires user gesture context).

(function () {
    // Create full-screen overlay with input at top
    const overlay = document.createElement('div');
    overlay.id = 'vkb-overlay';
    const input = document.createElement('input');
    input.type = 'text';
    input.autocapitalize = 'off';
    input.autocomplete = 'off';
    input.autocorrect = 'off';
    input.spellcheck = false;
    overlay.appendChild(input);
    document.body.appendChild(overlay);

    let active = false;
    // Pending open request from Rust (consumed by next touchend)
    let pendingOpen = null;
    // Cooldown after submit to prevent Rust from immediately re-opening
    let submitCooldown = false;

    function configureInput(detail) {
        const fieldType = detail.type || 'text';
        switch (fieldType) {
            case 'integer':
                input.type = 'text';
                input.inputMode = 'numeric';
                break;
            case 'decimal':
                input.type = 'text';
                input.inputMode = 'decimal';
                break;
            case 'email':
                input.type = 'email';
                input.inputMode = 'email';
                break;
            case 'password':
                input.type = 'password';
                input.inputMode = 'text';
                break;
            default:
                input.type = 'text';
                input.inputMode = 'text';
                break;
        }

        input.value = detail.value || '';
        if (detail.min != null) input.min = detail.min;
        else input.removeAttribute('min');
        if (detail.max != null) input.max = detail.max;
        else input.removeAttribute('max');
        input.required = !!detail.required;
    }

    function submit() {
        if (!active) return;
        active = false;
        submitCooldown = true;
        setTimeout(function () { submitCooldown = false; }, 200);

        const value = input.value;

        // Hide and clear
        overlay.style.display = 'none';
        input.value = '';
        input.blur();

        // Send value back to Rust
        document.dispatchEvent(new CustomEvent('vkb-submit', { detail: { value } }));

        // Wake the app from idle — dispatch a synthetic pointer event on the canvas
        // so winit processes it and triggers a redraw
        const canvas = document.getElementById('canvas');
        if (canvas) {
            canvas.dispatchEvent(new PointerEvent('pointermove', { bubbles: true }));
        }
    }

    // Submit on Enter
    input.addEventListener('keydown', function (e) {
        if (e.key === 'Enter') {
            e.preventDefault();
            submit();
        }
    });

    // Submit on blur (tap outside the input but inside the overlay)
    input.addEventListener('blur', function () {
        setTimeout(submit, 50);
    });

    // Tap on overlay background (outside input) dismisses
    overlay.addEventListener('touchend', function (e) {
        if (e.target === overlay && active) {
            submit();
        }
    });

    // Rust dispatches vkb-open: show overlay, configure input, wait for touchend to focus
    // Ignore if already active (Rust fires this every frame while wants_keyboard is true)
    document.addEventListener('vkb-open', function (e) {
        if (submitCooldown || active) return;
        pendingOpen = e.detail;
        configureInput(e.detail);
        overlay.style.display = 'block';
        active = true;
        input.focus();
    });

    // Focus the input on next touchend (iOS requires user gesture context for keyboard)
    document.addEventListener('touchend', function () {
        if (pendingOpen) {
            pendingOpen = null;
            input.focus();
        }
    });
})();
