## Stroke font advances

[`acadrust_shx`] is Acadrust Stroke, the single-stroke font the renderer uses when a text style's SHX or TrueType file is missing. Cap height is 72 font units. Descenders reach 20 units below the baseline. Rounds overshoot the cap and the baseline by about 2. The advance is the width of the character, in those same units, and it includes the side bearings. The ink of the letter can be narrower than the advance.

A text entity of height `h` and width factor `w` draws one font unit as `h / 72`. The cursor then moves `advance * h / 72 * w` drawing units. Digits all advance 52, so a column of dimensions lines up. An accented letter keeps the advance of its base letter.

### Matching a drawn glyph to a character

The renderer selects a glyph by the character's Unicode scalar value. `A` is shape 65 (`U+0041`). The shape number in the SHX file is that scalar. ASCII capitals and digits are named by the character (`A`, `0`). Every other shape is named `U` plus four hexadecimal digits (`U0061` for `a`).

An SVG drawing uses the same identity. Each character is one group. The group id is the code, and `data-advance` is the width from the table below. The position of the group on the page is not used, and the bounding box of the paths is not the advance.

```xml
<svg xmlns="http://www.w3.org/2000/svg" data-cap="72" data-descender="20">
  <g id="guides">
    <!-- baseline, cap line, side bearings; not a character -->
  </g>
  <g id="U+0041" data-advance="64">
    <path d="M 4,72 L 32,0 L 60,72 M 16,48 L 48,48"/>
  </g>
  <g id="U+0020" data-advance="30"/>
</svg>
```

* The id is `U+` and four hexadecimal digits, the same value as the Code column. `A` through `Z` and `0` through `9` may instead use the character itself (`id="A"`), which is the SHP name of those shapes.
* `data-advance` is the font-unit width. An empty group is a blank of that width. Space is `U+0020` with advance 30.
* Paths in the group are the pen strokes of that character. A path outside a character group is not a glyph.
* A group whose id is `guides` is not a character.
* SVG y grows downward. The cap line is y = 0, the baseline is y = 72, and descenders reach y = 92. One SVG user unit is one font unit. In the SHX file, y grows upward from the baseline.

### Advance table

`data-advance` is font units. Code is the SVG id without the `U+` prefix, and it is the SHX shape number.

| Character | Code | SHP name | data-advance |
| --- | --- | --- | --- |
| space | U+0020 | U0020 | 30 |
| `!` | U+0021 | U0021 | 24 |
| `"` | U+0022 | U0022 | 24 |
| `#` | U+0023 | U0023 | 52 |
| `$` | U+0024 | U0024 | 60 |
| `%` | U+0025 | U0025 | 52 |
| `&` | U+0026 | U0026 | 56 |
| `'` | U+0027 | U0027 | 16 |
| `(` | U+0028 | U0028 | 28 |
| `)` | U+0029 | U0029 | 28 |
| `*` | U+002A | U002A | 44 |
| `+` | U+002B | U002B | 44 |
| `,` | U+002C | U002C | 20 |
| `-` | U+002D | U002D | 32 |
| `.` | U+002E | U002E | 20 |
| `/` | U+002F | U002F | 36 |
| `0` | U+0030 | 0 | 52 |
| `1` | U+0031 | 1 | 52 |
| `2` | U+0032 | 2 | 52 |
| `3` | U+0033 | 3 | 52 |
| `4` | U+0034 | 4 | 52 |
| `5` | U+0035 | 5 | 52 |
| `6` | U+0036 | 6 | 52 |
| `7` | U+0037 | 7 | 52 |
| `8` | U+0038 | 8 | 52 |
| `9` | U+0039 | 9 | 52 |
| `:` | U+003A | U003A | 20 |
| `;` | U+003B | U003B | 20 |
| `<` | U+003C | U003C | 44 |
| `=` | U+003D | U003D | 44 |
| `>` | U+003E | U003E | 44 |
| `?` | U+003F | U003F | 36 |
| `@` | U+0040 | U0040 | 60 |
| `A` | U+0041 | A | 64 |
| `B` | U+0042 | B | 56 |
| `C` | U+0043 | C | 66 |
| `D` | U+0044 | D | 62 |
| `E` | U+0045 | E | 52 |
| `F` | U+0046 | F | 50 |
| `G` | U+0047 | G | 68 |
| `H` | U+0048 | H | 60 |
| `I` | U+0049 | I | 32 |
| `J` | U+004A | J | 50 |
| `K` | U+004B | K | 58 |
| `L` | U+004C | L | 50 |
| `M` | U+004D | M | 72 |
| `N` | U+004E | N | 60 |
| `O` | U+004F | O | 68 |
| `P` | U+0050 | P | 52 |
| `Q` | U+0051 | Q | 70 |
| `R` | U+0052 | R | 58 |
| `S` | U+0053 | S | 60 |
| `T` | U+0054 | T | 56 |
| `U` | U+0055 | U | 64 |
| `V` | U+0056 | V | 60 |
| `W` | U+0057 | W | 76 |
| `X` | U+0058 | X | 56 |
| `Y` | U+0059 | Y | 56 |
| `Z` | U+005A | Z | 56 |
| `[` | U+005B | U005B | 26 |
| `\` | U+005C | U005C | 36 |
| `]` | U+005D | U005D | 26 |
| `^` | U+005E | U005E | 44 |
| `_` | U+005F | U005F | 48 |
| `` ` `` | U+0060 | U0060 | 20 |
| `a` | U+0061 | U0061 | 50 |
| `b` | U+0062 | U0062 | 56 |
| `c` | U+0063 | U0063 | 52 |
| `d` | U+0064 | U0064 | 56 |
| `e` | U+0065 | U0065 | 54 |
| `f` | U+0066 | U0066 | 46 |
| `g` | U+0067 | U0067 | 50 |
| `h` | U+0068 | U0068 | 56 |
| `i` | U+0069 | U0069 | 28 |
| `j` | U+006A | U006A | 28 |
| `k` | U+006B | U006B | 52 |
| `l` | U+006C | U006C | 28 |
| `m` | U+006D | U006D | 68 |
| `n` | U+006E | U006E | 56 |
| `o` | U+006F | U006F | 56 |
| `p` | U+0070 | U0070 | 56 |
| `q` | U+0071 | U0071 | 56 |
| `r` | U+0072 | U0072 | 42 |
| `s` | U+0073 | U0073 | 50 |
| `t` | U+0074 | U0074 | 42 |
| `u` | U+0075 | U0075 | 56 |
| `v` | U+0076 | U0076 | 52 |
| `w` | U+0077 | U0077 | 64 |
| `x` | U+0078 | U0078 | 48 |
| `y` | U+0079 | U0079 | 52 |
| `z` | U+007A | U007A | 48 |
| `{` | U+007B | U007B | 30 |
| `\|` | U+007C | U007C | 16 |
| `}` | U+007D | U007D | 30 |
| `~` | U+007E | U007E | 42 |
| no-break space | U+00A0 | U00A0 | 30 |
| `¡` | U+00A1 | U00A1 | 24 |
| `¢` | U+00A2 | U00A2 | 48 |
| `£` | U+00A3 | U00A3 | 50 |
| `¥` | U+00A5 | U00A5 | 56 |
| `§` | U+00A7 | U00A7 | 48 |
| `«` | U+00AB | U00AB | 52 |
| soft hyphen | U+00AD | U00AD | 32 |
| `°` | U+00B0 | U00B0 | 24 |
| `±` | U+00B1 | U00B1 | 48 |
| `²` | U+00B2 | U00B2 | 30 |
| `³` | U+00B3 | U00B3 | 30 |
| `µ` | U+00B5 | U00B5 | 56 |
| `¶` | U+00B6 | U00B6 | 48 |
| `·` | U+00B7 | U00B7 | 18 |
| `¹` | U+00B9 | U00B9 | 30 |
| `»` | U+00BB | U00BB | 52 |
| `¼` | U+00BC | U00BC | 52 |
| `½` | U+00BD | U00BD | 52 |
| `¾` | U+00BE | U00BE | 52 |
| `¿` | U+00BF | U00BF | 36 |
| `À` | U+00C0 | U00C0 | 64 |
| `Á` | U+00C1 | U00C1 | 64 |
| `Â` | U+00C2 | U00C2 | 64 |
| `Ã` | U+00C3 | U00C3 | 64 |
| `Ä` | U+00C4 | U00C4 | 64 |
| `Å` | U+00C5 | U00C5 | 64 |
| `Æ` | U+00C6 | U00C6 | 68 |
| `Ç` | U+00C7 | U00C7 | 66 |
| `È` | U+00C8 | U00C8 | 52 |
| `É` | U+00C9 | U00C9 | 52 |
| `Ê` | U+00CA | U00CA | 52 |
| `Ë` | U+00CB | U00CB | 52 |
| `Ì` | U+00CC | U00CC | 32 |
| `Í` | U+00CD | U00CD | 32 |
| `Î` | U+00CE | U00CE | 32 |
| `Ï` | U+00CF | U00CF | 32 |
| `Ð` | U+00D0 | U00D0 | 58 |
| `Ñ` | U+00D1 | U00D1 | 60 |
| `Ò` | U+00D2 | U00D2 | 68 |
| `Ó` | U+00D3 | U00D3 | 68 |
| `Ô` | U+00D4 | U00D4 | 68 |
| `Õ` | U+00D5 | U00D5 | 68 |
| `Ö` | U+00D6 | U00D6 | 68 |
| `×` | U+00D7 | U00D7 | 48 |
| `Ø` | U+00D8 | U00D8 | 68 |
| `Ù` | U+00D9 | U00D9 | 64 |
| `Ú` | U+00DA | U00DA | 64 |
| `Û` | U+00DB | U00DB | 64 |
| `Ü` | U+00DC | U00DC | 64 |
| `Ý` | U+00DD | U00DD | 56 |
| `Þ` | U+00DE | U00DE | 52 |
| `ß` | U+00DF | U00DF | 52 |
| `à` | U+00E0 | U00E0 | 50 |
| `á` | U+00E1 | U00E1 | 50 |
| `â` | U+00E2 | U00E2 | 50 |
| `ã` | U+00E3 | U00E3 | 50 |
| `ä` | U+00E4 | U00E4 | 50 |
| `å` | U+00E5 | U00E5 | 50 |
| `æ` | U+00E6 | U00E6 | 56 |
| `ç` | U+00E7 | U00E7 | 52 |
| `è` | U+00E8 | U00E8 | 54 |
| `é` | U+00E9 | U00E9 | 54 |
| `ê` | U+00EA | U00EA | 54 |
| `ë` | U+00EB | U00EB | 54 |
| `ì` | U+00EC | U00EC | 28 |
| `í` | U+00ED | U00ED | 28 |
| `î` | U+00EE | U00EE | 28 |
| `ï` | U+00EF | U00EF | 28 |
| `ð` | U+00F0 | U00F0 | 58 |
| `ñ` | U+00F1 | U00F1 | 56 |
| `ò` | U+00F2 | U00F2 | 56 |
| `ó` | U+00F3 | U00F3 | 56 |
| `ô` | U+00F4 | U00F4 | 56 |
| `õ` | U+00F5 | U00F5 | 56 |
| `ö` | U+00F6 | U00F6 | 56 |
| `÷` | U+00F7 | U00F7 | 48 |
| `ø` | U+00F8 | U00F8 | 56 |
| `ù` | U+00F9 | U00F9 | 56 |
| `ú` | U+00FA | U00FA | 56 |
| `û` | U+00FB | U00FB | 56 |
| `ü` | U+00FC | U00FC | 56 |
| `ý` | U+00FD | U00FD | 52 |
| `þ` | U+00FE | U00FE | 50 |
| `ÿ` | U+00FF | U00FF | 52 |
| `Δ` | U+0394 | U0394 | 64 |
| `Ω` | U+03A9 | U03A9 | 72 |
| `α` | U+03B1 | U03B1 | 50 |
| `β` | U+03B2 | U03B2 | 46 |
| `γ` | U+03B3 | U03B3 | 50 |
| `θ` | U+03B8 | U03B8 | 54 |
| `μ` | U+03BC | U03BC | 56 |
| `π` | U+03C0 | U03C0 | 52 |
| `φ` | U+03C6 | U03C6 | 52 |
| `–` | U+2013 | U2013 | 32 |
| `—` | U+2014 | U2014 | 52 |
| `‘` | U+2018 | U2018 | 16 |
| `’` | U+2019 | U2019 | 16 |
| `“` | U+201C | U201C | 32 |
| `”` | U+201D | U201D | 34 |
| `•` | U+2022 | U2022 | 28 |
| `…` | U+2026 | U2026 | 52 |
| `′` | U+2032 | U2032 | 22 |
| `″` | U+2033 | U2033 | 32 |
| `€` | U+20AC | U20AC | 58 |
| `∅` | U+2205 | U2205 | 52 |
| `⌀` | U+2300 | U2300 | 52 |
