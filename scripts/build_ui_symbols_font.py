#!/usr/bin/env python3
"""Build assets/fonts/UISymbols.ttf: the characters the UI draws that
egui's own fonts cannot, cut from the repository's Noto fonts.

    python scripts/build_ui_symbols_font.py          # rebuild it
    python scripts/build_ui_symbols_font.py --check  # list what is missing

egui draws every platform's UI -- Windows, macOS, Linux and the web -- from
the fonts it embeds (Ubuntu Light, Noto Emoji, its icon font, Hack), and a
character none of them has is a square. The desktop also loads Noto Sans
from disk; the web does not. So the font the UI's symbols come from is
embedded too (`ui::font_loader`, `include_bytes!`), the same everywhere,
and holds only what egui lacks: arrows, check marks, geometric shapes,
mathematical symbols, sub- and superscripts, the language picker's own
names (日本語, 简体中文).

What the UI draws is read from the source, as `ui::font_loader`'s test
reads it: string literals and `\\u{..}` escapes in src/ (test modules,
`src/bin` and comments left out) and the values of locales/en.yml. Each
character egui's fonts lack is taken from the first of Noto Sans, Noto
Sans JP and Noto Sans SC that has it; one none has is reported, to be
replaced in the source with one that is drawn (egui's icon font has many).

The Noto fonts are under the SIL Open Font License (assets/fonts/OFL.txt),
which allows a subset to be embedded; the subset is renamed, as the
licence asks of a modified font. Needs fontTools (`pip install fonttools`).
"""
import glob
import os
import re
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.cu2quPen import Cu2QuPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
OUT = os.path.join(ROOT, 'assets', 'fonts', 'UISymbols.ttf')
SOURCES = ['NotoSans-Regular.otf', 'NotoSansJP-Regular.otf', 'NotoSansSC-Regular.otf']
# Characters that are not drawn: variation selectors and the joiner (the
# i18n test keeps them out of the translations), and controls.
INVISIBLE = {0xFE0E, 0xFE0F, 0x200D, 0x200B}


def egui_fonts():
    """egui's embedded fonts, from the cargo registry (the version the lock
    file names)."""
    lock = open(os.path.join(ROOT, 'Cargo.lock'), encoding='utf-8').read()
    m = re.search(r'name = "epaint_default_fonts"\nversion = "([^"]+)"', lock)
    pattern = os.path.expanduser(f'~/.cargo/registry/src/*/epaint_default_fonts-{m.group(1)}/fonts')
    found = glob.glob(pattern)
    if not found:
        sys.exit(f'egui\'s fonts not found ({pattern}): build the crate once')
    d = found[0]
    return [os.path.join(d, f) for f in ('Ubuntu-Light.ttf', 'NotoEmoji-Regular.ttf', 'emoji-icon-font.ttf', 'Hack-Regular.ttf')]


def cmap(path):
    return set(TTFont(path, fontNumber=0).getBestCmap())


def drawn_characters():
    """Every non-ASCII character the UI may draw, and one place each is."""
    found = {}
    lit = re.compile(r'"((?:[^"\\]|\\.)*)"')
    for path in glob.glob(os.path.join(ROOT, 'src', '**', '*.rs'), recursive=True):
        rel = os.path.relpath(path, ROOT).replace(os.sep, '/')
        if rel.startswith('src/bin/'):
            continue
        text = open(path, encoding='utf-8').read()
        # Test modules sit at the end of a file in this codebase.
        text = text.split('#[cfg(test)]')[0]
        for n, line in enumerate(text.splitlines(), 1):
            if line.lstrip().startswith('//'):
                continue
            for m in lit.finditer(line):
                s = re.sub(r'\\u\{([0-9A-Fa-f]{1,6})\}', lambda e: chr(int(e.group(1), 16)), m.group(1))
                for ch in s:
                    if ord(ch) > 127:
                        found.setdefault(ch, f'{rel}:{n}')
    for n, line in enumerate(open(os.path.join(ROOT, 'locales', 'en.yml'), encoding='utf-8'), 1):
        if not line.lstrip().startswith('#'):
            for ch in line:
                if ord(ch) > 127:
                    found.setdefault(ch, f'locales/en.yml:{n}')
    return {ch: w for ch, w in found.items() if ord(ch) not in INVISIBLE and ord(ch) >= 0x20}


def main():
    check = '--check' in sys.argv
    egui = set().union(*(cmap(p) for p in egui_fonts()))
    wanted = {ch: w for ch, w in drawn_characters().items() if ord(ch) not in egui}
    sources = [(name, os.path.join(ROOT, 'assets', 'fonts', name)) for name in SOURCES]
    plan, nowhere = {}, []
    for ch in sorted(wanted):
        src = next((name for name, path in sources if ord(ch) in cmap_cache(path)), None)
        if src:
            plan.setdefault(src, []).append(ch)
        else:
            nowhere.append(ch)
    for name, chars in plan.items():
        print(f'{name}: {"".join(chars)}')
    if nowhere:
        print('in no font -- replace these in the source:', file=sys.stderr)
        for ch in nowhere:
            print(f'  U+{ord(ch):04X} {ch}  {wanted[ch]}', file=sys.stderr)
    if check:
        return 1 if nowhere else 0

    build(plan, dict(sources))
    return 1 if nowhere else 0


def build(plan, paths):
    """The font, made fresh (fontTools' FontBuilder) rather than merged:
    the CJK sources are CID-keyed CFF, which fontTools cannot merge. Each
    glyph is drawn from its source into TrueType quadratics (cu2qu, as
    fontTools' otf2ttf does), its advance kept. Every source is a Noto
    font of 1000 units an em; the vertical metrics are Noto Sans's."""
    names, cmap, glyphs, metrics = ['.notdef'], {}, {}, {}
    empty = TTGlyphPen(None).glyph()
    glyphs['.notdef'] = empty
    metrics['.notdef'] = (500, 0)
    for source, chars in plan.items():
        font = TTFont(paths[source])
        assert font['head'].unitsPerEm == 1000, source
        glyph_set = font.getGlyphSet()
        best = font.getBestCmap()
        for ch in chars:
            src_name = best[ord(ch)]
            name = f'uni{ord(ch):04X}'
            pen = TTGlyphPen(None)
            glyph_set[src_name].draw(Cu2QuPen(pen, max_err=1.0, reverse_direction=True))
            glyphs[name] = pen.glyph()
            bounds = BoundsPen(glyph_set)
            glyph_set[src_name].draw(bounds)
            lsb = int(round(bounds.bounds[0])) if bounds.bounds else 0
            metrics[name] = (font['hmtx'][src_name][0], lsb)
            names.append(name)
            cmap[ord(ch)] = name
    latin = TTFont(paths['NotoSans-Regular.otf'])
    hhea, os2 = latin['hhea'], latin['OS/2']
    fb = FontBuilder(1000, isTTF=True)
    fb.setupGlyphOrder(names)
    fb.setupCharacterMap(cmap)
    fb.setupGlyf(glyphs)
    fb.setupHorizontalMetrics(metrics)
    fb.setupHorizontalHeader(ascent=hhea.ascent, descent=hhea.descent)
    # A modified font takes another name (the OFL's Reserved Font Name rule).
    fb.setupNameTable({
        'familyName': 'UISymbols',
        'styleName': 'Regular',
        'uniqueFontIdentifier': 'UISymbols-Regular',
        'fullName': 'UISymbols Regular',
        'psName': 'UISymbols-Regular',
        'copyright': 'Glyphs from Noto Sans, Noto Sans JP and Noto Sans SC, Copyright Google LLC, under the SIL Open Font License 1.1 (assets/fonts/OFL.txt).',
        'licenseDescription': 'SIL Open Font License, Version 1.1',
    })
    fb.setupOS2(
        sTypoAscender=os2.sTypoAscender,
        sTypoDescender=os2.sTypoDescender,
        sTypoLineGap=os2.sTypoLineGap,
        usWinAscent=os2.usWinAscent,
        usWinDescent=os2.usWinDescent,
    )
    fb.setupPost()
    fb.save(OUT)
    covered = set(TTFont(OUT).getBestCmap())
    lost = [ch for chars in plan.values() for ch in chars if ord(ch) not in covered]
    print(f'wrote {os.path.relpath(OUT, ROOT)}: {len(covered)} characters, {os.path.getsize(OUT):,} bytes')
    if lost:
        sys.exit(f'lost: {"".join(lost)}')


_cmaps = {}


def cmap_cache(path):
    if path not in _cmaps:
        _cmaps[path] = cmap(path)
    return _cmaps[path]


if __name__ == '__main__':
    raise SystemExit(main())
