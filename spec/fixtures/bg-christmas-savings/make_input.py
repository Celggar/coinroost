"""Writes input.pdf: a synthetic Banco General Christmas-savings statement (every name, reference and amount
is invented).

Standard library only, deterministic output. The statement is one table drawn with ruling lines: Fecha,
Descripción, Monto (signed, with a dollar sign) and Saldo. Descriptions take up to three lines in their cell,
dates use Spanish month abbreviations, and the table continues on a second page under a repeated header. The
first row is the opening balance: no date, and one cell across Descripción and Monto. Two rows are broken on
purpose (an impossible date, an empty amount) and must be reported.

Run: python3 spec/fixtures/bg-christmas-savings/make_input.py [options] [folder]

The options draw the same statement in other ways, to check that a reader finds the same table:
  --grid lines        each rule a separate stroked line (default)
  --grid cells        one stroked rectangle per cell
  --grid thin         rules as filled rectangles half a point thick
  --grid single-path  every rule of a page in one stroked path
  --grid fills        row shading only, no rules (no columns can be found)
  --scale             everything under a `cm` that scales by 0.24 and shifts by 36.85 points
  --origin X,Y        MediaBox origin (default 0,0)
  --rotate N          page /Rotate
  --extras            a zero-height rectangle on the top rule, a sloped line, a frame with round corners
  --blank-header N    header cell N left empty
  --jitter            rules that stop short of each other, columns off by under a point, doubled rules
"""
import argparse
from pathlib import Path

FONT_SIZE = 8
CHAR_WIDTH = 0.6 * FONT_SIZE  # Courier: every glyph is 600/1000 em wide
PAGE_W, PAGE_H = 612, 792
COLUMN_X = [40, 115, 395, 485, 572]  # left side of each column, then the right side of the table
LINE_STEP = 10
PAD = 4
SCALE, SHIFT = 0.24, 36.85

HEADER_BLOCK = [
    "BANCO GENERAL, S.A.",
    "ESTADO DE CUENTA - AHORROS NAVIDAD",
    "Cliente: JANE DOE EXAMPLE",
    "Cuenta: 00-00-00-000002",
    "Periodo: 01-jun-2026 al 30-sep-2026",
    "Saldo total: $849.99",
]
COLUMNS = ["Fecha", "Descripción", "Monto", "Saldo"]
# (date, description lines, amount, balance); amount None: the description cell spans the Monto column.
OPENING = ("", ["SALDO INICIAL"], None, "$0.00")
ROWS = [
    ("05-jun-2026", ["DEPOSITO POR TRANSFERENCIA", "DESDE CUENTA 00-00-00-000001", "REF 7001"], "$150.00", "$150.00"),
    ("20-jun-2026", ["ABONO PROGRAMADO AHORRO", "NAVIDENO"], "$100.00", "$250.00"),
    ("20-jun-2026", ["ABONO PROGRAMADO AHORRO", "NAVIDENO"], "$100.00", "$350.00"),
    ("05-jul-2026", ["INTERESES GANADOS"], "$0.37", "$350.37"),
    ("20-jul-2026", ["ABONO PROGRAMADO AHORRO", "NAVIDENO"], "$100.00", "$450.37"),
    ("02-ago-2026", ["DEPOSITO EN EFECTIVO", "SUCURSAL EJEMPLO"], "$2,500.00", "$2,950.37"),
    ("20-ago-2026", ["ABONO PROGRAMADO AHORRO", "NAVIDENO"], "$100.00", "$3,050.37"),
    ("05-sep-2026", ["INTERESES GANADOS"], "$1.12", "$3,051.49"),
    ("15-sep-2026", ["RETIRO PARCIAL A CUENTA", "PROPIA 00-00-00-000001", "SOLICITADO EN LINEA"], "-$2,300.00", "$751.49"),
    ("20-sep-2026", ["ABONO PROGRAMADO AHORRO", "NAVIDENO"], "$100.00", "$851.49"),
    ("30-sep-2026", ["CARGO POR SERVICIO"], "-$1.50", "$849.99"),
    ("31-sep-2026", ["MOVIMIENTO CON FECHA", "IMPOSIBLE"], "$5.00", "$854.99"),
    ("30-sep-2026", ["MOVIMIENTO SIN MONTO"], "", "$854.99"),
]
PAGES = [[OPENING] + ROWS[:7], ROWS[7:]]


def number(v: float) -> bytes:
    return (("%.4f" % v).rstrip("0").rstrip(".") or "0").encode()


def escape(text: str) -> bytes:
    raw = text.encode("cp1252")
    return raw.replace(b"\\", b"\\\\").replace(b"(", b"\\(").replace(b")", b"\\)")


class Canvas:
    """Content stream in page points (y up), written through the chosen origin and scaling."""

    def __init__(self, origin: tuple, scale: bool):
        self.ox, self.oy = origin
        self.scale = scale
        self.out = bytearray()

    def at(self, x: float, y: float) -> bytes:
        x, y = x + self.ox, y + self.oy
        if self.scale:
            x, y = (x - SHIFT) / SCALE, y / SCALE
        return number(x) + b" " + number(y)

    def length(self, v: float) -> bytes:
        return number(v / SCALE if self.scale else v)

    def text(self, x: float, y: float, text: str) -> None:
        size = self.length(FONT_SIZE)
        self.out += b"BT /F1 " + size + b" Tf " + self.at(x, y) + b" Td (" + escape(text) + b") Tj ET\n"

    def right(self, x_end: float, y: float, text: str) -> None:
        self.text(x_end - CHAR_WIDTH * len(text), y, text)

    def segment(self, x0: float, y0: float, x1: float, y1: float) -> bytes:
        return self.at(x0, y0) + b" m " + self.at(x1, y1) + b" l\n"

    def rect(self, x: float, y: float, w: float, h: float) -> bytes:
        return self.at(x, y) + b" " + self.length(w) + b" " + self.length(h) + b" re\n"

    def raw(self, ops: bytes) -> None:
        self.out += ops

    def stream(self) -> bytes:
        if not self.scale:
            return bytes(self.out)
        return b"q %s 0 0 %s %s 0 cm\n" % (number(SCALE), number(SCALE), number(SHIFT)) + bytes(self.out) + b"Q\n"


def row_height(row: tuple) -> float:
    return PAD + LINE_STEP * len(row[1]) + PAD - 2


def spans(row: tuple) -> list:
    """Cell spans of a row as (left, right) pairs."""
    xs = COLUMN_X if row[2] is not None else COLUMN_X[:2] + COLUMN_X[3:]
    return list(zip(xs, xs[1:]))


def draw_grid(c: Canvas, rows: list, top: float, grid: str, jitter: bool) -> None:
    """Rules for the header row plus `rows`, whose top edge is at `top`."""
    left, right = COLUMN_X[0], COLUMN_X[-1]
    bounds, y = [], top
    for row in rows:
        bounds.append((y, y - row_height(row), row))
        y -= row_height(row)
    if grid == "fills":
        for upper, lower, _ in bounds:
            c.raw(b"0.92 g\n" + c.rect(left, lower, right - left, upper - lower) + b"f\n0 g\n")
        return
    if grid == "cells":
        for upper, lower, row in bounds:
            for a, b in spans(row):
                c.raw(c.rect(a, lower, b - a, upper - lower) + b"S\n")
        return
    rules_y = [top] + [lower for _, lower, _ in bounds]
    verticals = [(x, upper, lower) for upper, lower, row in bounds for x in [a for a, _ in spans(row)] + [right]]
    if grid == "thin":
        for y in rules_y:
            c.raw(c.rect(left, y - 0.25, right - left, 0.5) + b"f\n")
        for x, upper, lower in verticals:
            c.raw(c.rect(x - 0.25, lower, 0.5, upper - lower) + b"f\n")
        return
    if jitter:
        # Within pdfplumber's 3-point tolerances: vertical rules stop 1.2 points short of the horizontal
        # ones (and break in the middle of tall cells), every other column is drawn 0.8 points to the right,
        # horizontal rules are inset 1.5 points and every third one is doubled 0.6 points lower.
        shift = {x: 0.8 * (i % 2) for i, x in enumerate(COLUMN_X)}
        segments = [c.segment(left + 1.5, y, right - 1.5, y) for y in rules_y]
        segments += [c.segment(left, y - 0.6, right, y - 0.6) for y in rules_y[::3]]
        for x, upper, lower in verticals:
            x += shift[x]
            if upper - lower > 20:  # broken in the middle of a tall cell, 2 points apart
                middle = (upper + lower) / 2
                segments += [c.segment(x, upper - 1.2, x, middle + 1), c.segment(x, middle - 1, x, lower + 1.2)]
            else:
                segments.append(c.segment(x, upper - 1.2, x, lower + 1.2))
    else:
        segments = [c.segment(left, y, right, y) for y in rules_y]
        segments += [c.segment(x, upper, x, lower) for x, upper, lower in verticals]
    if grid == "single-path":
        c.raw(b"".join(segments) + b"S\n")
    else:
        for s in segments:
            c.raw(s + b"S\n")


def draw_row(c: Canvas, row: tuple, upper: float) -> None:
    date, lines, amount, balance = row
    baseline = upper - PAD - 7
    if date:
        c.text(COLUMN_X[0] + PAD, baseline, date)
    for i, line in enumerate(lines):
        c.text(COLUMN_X[1] + PAD, baseline - LINE_STEP * i, line)
    if amount:
        c.right(COLUMN_X[3] - PAD, baseline, amount)
    c.right(COLUMN_X[4] - PAD, baseline, balance)


def page_stream(rows: list, number_: int, args) -> bytes:
    c = Canvas(args.origin, args.scale)
    y = PAGE_H - 50
    block = HEADER_BLOCK if number_ == 1 else [HEADER_BLOCK[1] + " (continuación)"]
    for line in block:
        c.text(40, y, line)
        y -= 14
    if args.extras and number_ == 1:
        c.raw(c.segment(400, PAGE_H - 60, 460, PAGE_H - 40) + b"S\n")
        r, x0, x1, y0, y1 = 4, 34, 300, y + 6, PAGE_H - 36
        frame = c.at(x0 + r, y0) + b" m " + c.at(x1 - r, y0) + b" l "
        frame += c.at(x1, y0) + b" " + c.at(x1, y0) + b" " + c.at(x1, y0 + r) + b" c "
        frame += c.at(x1, y1 - r) + b" l " + c.at(x1, y1) + b" " + c.at(x1, y1) + b" " + c.at(x1 - r, y1) + b" c "
        frame += c.at(x0 + r, y1) + b" l " + c.at(x0, y1) + b" " + c.at(x0, y1) + b" " + c.at(x0, y1 - r) + b" c "
        frame += c.at(x0, y0 + r) + b" l " + c.at(x0, y0) + b" " + c.at(x0, y0) + b" " + c.at(x0 + r, y0) + b" c h S\n"
        c.raw(frame)
    top = y - 6
    header = ("", [""], "", "")
    draw_grid(c, [header] + rows, top, args.grid, args.jitter)
    if args.extras:
        c.raw(c.rect(COLUMN_X[0], top, COLUMN_X[-1] - COLUMN_X[0], 0) + b"S\n")
    baseline = top - PAD - 7
    for i, (x, name) in enumerate(zip(COLUMN_X, COLUMNS)):
        if i != args.blank_header:
            c.text(x + PAD, baseline, name)
    upper = top - row_height(header)
    for row in rows:
        draw_row(c, row, upper)
        upper -= row_height(row)
    c.text(40, 40, "Página %d de %d" % (number_, len(PAGES)))
    return c.stream()


def build(args) -> bytes:
    pages = [page_stream(rows, i + 1, args) for i, rows in enumerate(PAGES)]
    n_pages = len(pages)
    font_id = 3 + 2 * n_pages
    ox, oy = args.origin
    media = b"[%s %s %s %s]" % (number(ox), number(oy), number(ox + PAGE_W), number(oy + PAGE_H))
    rotate = b" /Rotate %d" % args.rotate if args.rotate else b""
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [%s] /Count %d >>"
        % (b" ".join(b"%d 0 R" % (3 + 2 * i) for i in range(n_pages)), n_pages),
    ]
    for i, stream in enumerate(pages):
        objects.append(
            b"<< /Type /Page /Parent 2 0 R /MediaBox %s%s /Resources << /Font << /F1 %d 0 R >> >> "
            b"/Contents %d 0 R >>" % (media, rotate, font_id, 4 + 2 * i)
        )
        objects.append(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"endstream")
    widths = b" ".join([b"600"] * 224)
    objects.append(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding "
        b"/FirstChar 32 /LastChar 255 /Widths [" + widths + b"] >>"
    )
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for n, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % n + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
    out += b"".join(b"%010d 00000 n \n" % o for o in offsets)
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)
    return bytes(out)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("folder", nargs="?", type=Path, default=Path(__file__).parent)
    parser.add_argument("--grid", choices=["lines", "cells", "thin", "single-path", "fills"], default="lines")
    parser.add_argument("--scale", action="store_true")
    parser.add_argument("--origin", type=lambda s: tuple(float(v) for v in s.split(",")), default=(0, 0))
    parser.add_argument("--rotate", type=int, default=0)
    parser.add_argument("--extras", action="store_true")
    parser.add_argument("--blank-header", type=int, default=-1)
    parser.add_argument("--jitter", action="store_true")
    args = parser.parse_args()
    target = args.folder / "input.pdf"
    target.write_bytes(build(args))
    print(f"wrote {target} ({target.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
