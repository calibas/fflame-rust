// The built-in Field Value colouring as a field colouring of your own (the
// Escape panel's Edit as Rhai): the palette position straight from the
// field's sum.

const PARAMS = #{
    scale: #{ value: 1.0, min: 0.01, max: 20.0, caption: "Scale", hint: "Palette cycles per unit of field value." },
    offset: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Offset", hint: "Palette phase offset." },
};

const INFO = #{ title: "Field Value" };

fn shade(sum, grad, n, p) {
    sum * p.scale + p.offset
}
