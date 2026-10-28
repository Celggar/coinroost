"""Writes the two synthetic Banco General workbooks (all names, references and amounts are invented).

`bg-account/input.xlsx` has the older layout (one signed `Monto`); `bg-account-debit-credit/input.xlsx` the
accounting layout (`Débito` already negative, `Crédito`). Both have seven metadata rows above the header,
dates as text and as date cells, numbers as text and as number cells, and a blank row.
Requires openpyxl. Run: python3 spec/fixtures/bg-account/make_input.py
"""
import datetime
from pathlib import Path

import openpyxl

HERE = Path(__file__).parent
FIXED = datetime.datetime(2026, 1, 1)


def metadata(sheet, account_line: str) -> None:
    sheet.append([])
    sheet.append(["", "Últimos movimientos"])
    sheet.append(["", account_line])
    sheet.append(["", "Moneda: USD"])
    sheet.append([])
    sheet.append(["", "Desde: 01/06/2026 Hasta: 30/06/2026"])
    sheet.append([])


def save(book, path: Path) -> None:
    book.properties.created = FIXED
    book.properties.modified = FIXED
    book.save(path)


def signed_layout() -> None:
    book = openpyxl.Workbook()
    sheet = book.active
    sheet.title = "BGPExcelReport"
    metadata(sheet, "Cuenta:Ejemplo 00-00-00-000000-0")
    sheet.append(["", "Fecha", "Referencia", "Descripción", "Monto", "Saldo total"])
    sheet.append(["", "2026-06-26 23:49:57", "0", "TRANSFERENCIA A JOHN DOE", -8, 397.98])
    sheet.append(["", "2026-06-25 23:49:57", "0", "TRANSFERENCIA DE EMPRESA EJEMPLO", "279.53", "405.98"])
    sheet.append(["", datetime.datetime(2026, 6, 20, 10, 15, 0), "0", "COMPRA EJEMPLO", -12.5, 126.45])
    sheet.append([])
    sheet.append(["", "2026-06-18 08:00:00", "0", "", -1.25, 138.95])
    sheet.append(["", "18/06/2026", "0", "CARGO EJEMPLO", -3, 140.2])
    sheet.append(["", "fecha rara", "0", "MOVIMIENTO SIN FECHA", -1, 143.2])
    sheet.append(["", "2026-06-15 07:30:00", "0", "INTERES EJEMPLO", 0.125, 144.2])
    save(book, HERE / "input.xlsx")


def debit_credit_layout() -> None:
    book = openpyxl.Workbook()
    sheet = book.active
    sheet.title = "BGRExcelContReport"
    metadata(sheet, "Cuenta:Ejemplo 00-00-00-000000-1")
    sheet.append(["Fecha", "", "Referencia", "Transacción", "Descripción", "Débito", "Crédito", "", "Saldo total"])
    sheet.append(["2026-06-26 16:10:04", "", "0", "264", "BANCA MOVIL A OTRO BANCO", -800, None, None, 1000.79])
    sheet.append(["2026-05-31 23:51:43", "", "0", "221", "INTERES CUENTA DE AHORROS", None, 0.79, None, 1800.79])
    sheet.append(["2026-05-16 23:51:43", "", "0", "300", "TRANSFERENCIA ENTRE CUENTAS", "", "1300", "", "1800"])
    sheet.append([datetime.datetime(2026, 5, 10), "", "0", "300", "PAGO CON AMBOS", -10, 10, None, 500])
    sheet.append(["2026-05-09 09:00:00", "", "0", "300", "MOVIMIENTO SIN MONTO", None, None, None, 500])
    save(book, HERE.parent / "bg-account-debit-credit" / "input.xlsx")


if __name__ == "__main__":
    signed_layout()
    debit_credit_layout()
    print("wrote bg-account/input.xlsx and bg-account-debit-credit/input.xlsx")
