// Virtual Keyboard overlay for mobile WASM
// Shows a native input field with submit/cancel buttons above the canvas.
// The overlay covers the full screen to block touch events on the canvas.
// Focus happens in a touchend handler (iOS requires user gesture context).

(function () {
    // Create full-screen overlay with input bar at top
    const overlay = document.createElement('div');
    overlay.id = 'vkb-overlay';

    const bar = document.createElement('div');
    bar.className = 'vkb-bar';

    const input = document.createElement('input');
    input.type = 'text';
    input.autocapitalize = 'off';
    input.autocomplete = 'off';
    input.autocorrect = 'off';
    input.spellcheck = false;
    input.setAttribute('enterkeyhint', 'done');

    // Multiline twin (script editor and other TextEdit::multiline
    // fields). Swapped in by configureInput for type "multiline";
    // Enter inserts a newline there, so submit is button/blur only.
    const area = document.createElement('textarea');
    area.autocapitalize = 'off';
    area.autocomplete = 'off';
    area.autocorrect = 'off';
    area.spellcheck = false;
    area.style.display = 'none';

    // The element currently being edited (input or area).
    let field = input;

    const submitBtn = document.createElement('button');
    submitBtn.className = 'vkb-submit';
    submitBtn.textContent = '✓';
    submitBtn.type = 'button';

    const cancelBtn = document.createElement('button');
    cancelBtn.className = 'vkb-cancel';
    cancelBtn.textContent = '✕';
    cancelBtn.type = 'button';

    bar.appendChild(input);
    bar.appendChild(area);
    bar.appendChild(submitBtn);
    bar.appendChild(cancelBtn);
    overlay.appendChild(bar);
    document.body.appendChild(overlay);

    let active = false;
    // Pending open request from Rust (consumed by next touchend)
    let pendingOpen = null;
    // Cooldown after submit to prevent Rust from immediately re-opening
    let submitCooldown = false;
    // Set when blur is from a button click (don't auto-submit)
    let blurFromButton = false;

    function configureInput(detail) {
        const fieldType = detail.type || 'text';

        // Pick the element: textarea for multiline, input otherwise.
        const wantArea = fieldType === 'multiline';
        field = wantArea ? area : input;
        area.style.display = wantArea ? 'block' : 'none';
        input.style.display = wantArea ? 'none' : 'block';
        bar.classList.toggle('vkb-multiline', wantArea);
        if (wantArea) {
            area.value = detail.value || '';
            return;
        }

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

    function close() {
        active = false;
        submitCooldown = true;
        setTimeout(function () { submitCooldown = false; }, 200);

        overlay.style.display = 'none';
        input.value = '';
        area.value = '';
        field.blur();

        // Wake the app from idle — dispatch a synthetic pointer event on the canvas
        const canvas = document.getElementById('canvas');
        if (canvas) {
            canvas.dispatchEvent(new PointerEvent('pointermove', { bubbles: true }));
        }
    }

    function submit() {
        if (!active) return;
        const value = field.value;
        close();
        document.dispatchEvent(new CustomEvent('vkb-submit', { detail: { value } }));
    }

    function cancel() {
        if (!active) return;
        close();
        // No vkb-submit event — egui field stays unchanged
        // Still need to defocus the egui field on the Rust side
        document.dispatchEvent(new CustomEvent('vkb-cancel'));
    }

    // Buttons use mousedown/touchstart to fire BEFORE the input's blur event
    submitBtn.addEventListener('mousedown', function (e) {
        e.preventDefault();
        blurFromButton = true;
        submit();
    });
    submitBtn.addEventListener('touchstart', function (e) {
        e.preventDefault();
        blurFromButton = true;
        submit();
    });

    cancelBtn.addEventListener('mousedown', function (e) {
        e.preventDefault();
        blurFromButton = true;
        cancel();
    });
    cancelBtn.addEventListener('touchstart', function (e) {
        e.preventDefault();
        blurFromButton = true;
        cancel();
    });

    // Submit on Enter — single-line only; the textarea needs Enter for
    // newlines and submits via the ✓ button or blur instead.
    input.addEventListener('keydown', function (e) {
        if (e.key === 'Enter') {
            e.preventDefault();
            submit();
        }
    });

    // Submit on blur (tap outside the field but inside the overlay)
    // — but skip if blur is from a button click
    function blurSubmit() {
        setTimeout(function () {
            if (blurFromButton) {
                blurFromButton = false;
                return;
            }
            submit();
        }, 50);
    }
    input.addEventListener('blur', blurSubmit);
    area.addEventListener('blur', blurSubmit);

    // Tap on overlay background (outside input/buttons) cancels
    overlay.addEventListener('touchend', function (e) {
        if (e.target === overlay && active) {
            cancel();
        }
    });

    // Rust dispatches vkb-open: show overlay, configure input, wait for touchend to focus
    document.addEventListener('vkb-open', function (e) {
        if (submitCooldown || active) return;
        pendingOpen = e.detail;
        configureInput(e.detail);
        overlay.style.display = 'block';
        active = true;
        field.focus();
    });

    // Focus the field on next touchend (iOS requires user gesture context for keyboard)
    document.addEventListener('touchend', function () {
        if (pendingOpen) {
            pendingOpen = null;
            field.focus();
        }
    });
})();
