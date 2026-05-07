#!/usr/bin/env python3
"""Render a PDF page range to PNG images for visual question reading."""

from __future__ import annotations

import argparse
from pathlib import Path

import fitz  # PyMuPDF


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Render PDF pages to PNG images.")
    parser.add_argument("--pdf", required=True, help="Path to the source PDF.")
    parser.add_argument("--from", dest="from_page", type=int, required=True, help="First 1-based page number.")
    parser.add_argument("--to", dest="to_page", type=int, required=True, help="Last 1-based page number, inclusive.")
    parser.add_argument("--output", required=True, help="Output directory for rendered PNG files.")
    parser.add_argument("--scale", type=float, default=1.6, help="Render scale. Increase for small text. Default: 1.6")
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    pdf = Path(args.pdf)
    output = Path(args.output)
    if args.from_page <= 0 or args.to_page < args.from_page:
        raise SystemExit("Invalid page range. Use 1-based --from and --to with --to >= --from.")
    if not pdf.exists():
        raise SystemExit(f"PDF not found: {pdf}")

    output.mkdir(parents=True, exist_ok=True)
    doc = fitz.open(str(pdf))
    last = min(args.to_page, doc.page_count)
    matrix = fitz.Matrix(args.scale, args.scale)

    print(f"pdf={pdf}")
    print(f"page_count={doc.page_count}")
    print(f"rendering={args.from_page}-{last}")
    for page_number in range(args.from_page, last + 1):
        page = doc.load_page(page_number - 1)
        pix = page.get_pixmap(matrix=matrix, alpha=False)
        path = output / f"page-{page_number:03}.png"
        pix.save(str(path))
        print(path)


if __name__ == "__main__":
    main()
