// The built-in Hillshade colouring as a field colouring of your own (the
// Escape panel's Edit as Rhai): the field's value for the palette, its
// gradient a surface normal lit by a directional light.

const PARAMS = #{
    azimuth: #{
        value: 315.0, min: 0.0, max: 360.0,
        caption: "Light Azimuth",
        hint: "Light direction, degrees clockwise from north (the cartography convention; 315 = upper-left).",
    },
    elevation: #{ value: 45.0, min: 5.0, max: 90.0, caption: "Light Elevation", hint: "Light height above the horizon, degrees." },
    relief: #{ value: 1.0, min: 0.01, max: 20.0, caption: "Relief", hint: "Vertical exaggeration of the surface before lighting." },
    scale: #{ value: 1.0, min: 0.01, max: 20.0, caption: "Palette Scale", hint: "Palette cycles per unit of field value." },
};

const INFO = #{ title: "Hillshade" };

fn shade(sum, grad, n, p) {
    let az = p.azimuth * 0.01745329;
    let el = p.elevation * 0.01745329;
    let relief = p.relief;
    let normal = normalize(vec3(-grad.re * relief, -grad.im * relief, 1.0));
    let light = vec3(sin(az) * cos(el), cos(az) * cos(el), sin(el));
    let lum = clamp(dot(normal, light), 0.0, 1.0) * 0.9 + 0.1;
    #{ t: sum * p.scale, lum: lum }
}
