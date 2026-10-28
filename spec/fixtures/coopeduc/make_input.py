"""Writes input.pdf: a synthetic Coopeduc-style statement (all names, references and amounts are invented).

Standard library only, deterministic output. Layout follows the real export: a header block, then one line per
transaction with date, reference, detail, debit, credit and balance; the table header repeats on every page.
Run: python3 spec/fixtures/coopeduc/make_input.py
"""
from pathlib import Path

FONT_SIZE = 8
CHAR_WIDTH = 0.6 * FONT_SIZE  # Courier: every glyph is 600/1000 em wide
PAGE_W, PAGE_H = 612, 792
ROWS_PER_PAGE = 9

HEADER = [
    (150, "COOPERATIVA DE AHORRO Y CRÉDITO EL EDUCADOR, R.L."),
    (40, "Estado de Cuenta"),
    (40, "Nombre del Socio: JANE DOE EXAMPLE"),
    (40, "Nombre de la Cuenta: AH CORRIENTE"),
    (40, "Desde: 01/01/2025 Hasta: 31/03/2025"),
]
COLUMNS = [(40, "Fecha"), (100, "Referencia"), (170, "Detalle"), (430, "Débito"), (490, "Crédito"), (550, "Balance")]

# (date, reference, detail, debit, credit, balance)
ROWS = [
    ("02/01/2025", "70010001", "DEPOSITO EN EFECTIVO", "0.00", "250.00", "1,250.00"),
    ("05/01/2025", "70010002", "ACH-BANCO EJEMPLO-JANE DOE EXAMPLE", "0.00", "1,000.00", "2,250.00"),
    ("09/01/2025", "70010003", "RETIRO CAJERO AUTOMATICO", "40.00", "0.00", "2,210.00"),
    ("09/01/2025", "70010004", "RETIRO CAJERO AUTOMATICO", "40.00", "0.00", "2,170.00"),
    ("15/01/2025", "70010005", "CUOTA PRESTAMO PERSONAL", "125.35", "0.00", "2,044.65"),
    ("31/01/2025", "70010006", "INTERESES DE AHORRO", "0.00", "1.07", "2,045.72"),
    ("03/02/2025", "70010007", "TRANSFERENCIA A CUENTA PROPIA", "1,500.00", "0.00", "545.72"),
    ("14/02/2025", "70010008", "PAGO SEGURO DE VIDA", "12.50", "0.00", "533.22"),
    ("28/02/2025", "70010009", "INTERESES DE AHORRO", "0.00", "0.44", "533.66"),
    ("03/03/2025", "70010010", "DEPOSITO CHEQUE AJENO", "0.00", "2,345.67", "2,879.33"),
    ("10/03/2025", "70010011", "COMISION MANEJO DE CUENTA", "3.00", "0.00", "2,876.33"),
    ("17/03/2025", "70010012", "REVERSO DE COMISION", "0.00", "3.00", "2,879.33"),
    ("31/03/2025", "70010013", "INTERESES DE AHORRO", "0.00", "2.36", "2,881.69"),
]
# A line that starts with a date but does not have the three amounts: the normalizer must report it.
BROKEN_LINE = (40, "20/03/2025 70010014 MOVIMIENTO SIN MONTOS")


def escape(text: str) -> bytes:
    raw = text.encode("cp1252")
    return raw.replace(b"\\", b"\\\\").replace(b"(", b"\\(").replace(b")", b"\\)")


def show(x: float, y: float, text: str) -> bytes:
    return b"BT /F1 %d Tf %.2f %.2f Td (" % (FONT_SIZE, x, y) + escape(text) + b") Tj ET\n"


def right(x_end: float, y: float, text: str) -> bytes:
    return show(x_end - CHAR_WIDTH * len(text), y, text)


def page_stream(rows: list, first: bool, extra: list) -> bytes:
    out, y = b"", PAGE_H - 50
    if first:
        for x, text in HEADER:
            out += show(x, y, text)
            y -= 14
        y -= 6
    for x, text in COLUMNS:
        out += show(x, y, text)
    y -= 14
    for date, ref, detail, debit, credit, balance in rows:
        out += show(40, y, date) + show(100, y, ref) + show(170, y, detail)
        out += right(460, y, debit) + right(520, y, credit) + right(580, y, balance)
        y -= 12
    for x, text in extra:
        out += show(x, y, text)
        y -= 12
    return out + show(40, 40, "Página %d" % (2 if not first else 1))


def build() -> bytes:
    pages = [
        page_stream(ROWS[:ROWS_PER_PAGE], True, []),
        page_stream(ROWS[ROWS_PER_PAGE:], False, [BROKEN_LINE]),
    ]
    n_pages = len(pages)
    font_id = 3 + 2 * n_pages
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [%s] /Count %d >>"
        % (b" ".join(b"%d 0 R" % (3 + 2 * i) for i in range(n_pages)), n_pages),
    ]
    for i, stream in enumerate(pages):
        objects.append(
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 %d %d] /Resources << /Font << /F1 %d 0 R >> >> "
            b"/Contents %d 0 R >>" % (PAGE_W, PAGE_H, font_id, 4 + 2 * i)
        )
        objects.append(b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"endstream")
    widths = b" ".join([b"600"] * 224)
    objects.append(
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding "
        b"/FirstChar 32 /LastChar 255 /Widths [" + widths + b"] >>"
    )
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for number, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % number + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
    out += b"".join(b"%010d 00000 n \n" % o for o in offsets)
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)
    return bytes(out)


if __name__ == "__main__":
    target = Path(__file__).with_name("input.pdf")
    target.write_bytes(build())
    print(f"wrote {target} ({target.stat().st_size} bytes)")
