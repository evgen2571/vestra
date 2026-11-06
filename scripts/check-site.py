#!/usr/bin/env python3
"""Check generated HTML links, media and anchors under the configured site path."""

import argparse
from html.parser import HTMLParser
from pathlib import Path
import tomllib
from urllib.parse import unquote, urljoin, urlsplit


class Page(HTMLParser):
    def __init__(self, path: Path) -> None:
        super().__init__()
        self.ids: set[str] = set()
        self.links: list[tuple[int, str]] = []
        self.feed(path.read_text(encoding="utf-8"))

    def handle_starttag(self, tag: str, attrs: list[tuple[str, str | None]]) -> None:
        attributes = dict(attrs)
        identifier = attributes.get("id") or (
            attributes.get("name") if tag == "a" else None
        )
        if identifier:
            self.ids.add(identifier)
        for name in ("href", "src", "poster"):
            if attributes.get(name):
                self.links.append((self.getpos()[0], attributes[name]))


def check(site: Path, site_url: str) -> tuple[int, list[str]]:
    site = site.resolve()
    pages = {path: Page(path) for path in site.rglob("*.html")}
    if site / "index.html" not in pages:
        return 0, ["No generated index.html found; build the site first."]
    base = urlsplit(site_url)
    prefix = base.path.rstrip("/") + "/"
    errors = []
    checked = 0
    for source, page in pages.items():
        relative = source.relative_to(site)
        page_url = urljoin(site_url, str(relative))
        if source.name == "index.html":
            page_url = page_url.removesuffix("index.html")
        for line, target in page.links:
            parsed = urlsplit(urljoin(page_url, target))
            if parsed.scheme not in ("http", "https") or parsed.netloc != base.netloc:
                continue
            checked += 1
            location = f"{relative}:{line}: {target}"
            if not parsed.path.startswith(prefix):
                errors.append(f"{location} points outside {prefix}")
                continue
            destination = (site / unquote(parsed.path.removeprefix(prefix))).resolve()
            if not destination.is_relative_to(site):
                errors.append(f"{location} escapes the site directory")
                continue
            if destination.is_dir():
                destination /= "index.html"
            if not destination.is_file():
                errors.append(f"{location} has no generated target")
            elif parsed.fragment and destination in pages:
                fragment = unquote(parsed.fragment)
                if fragment not in pages[destination].ids:
                    errors.append(f"{location} has no matching anchor")
    return checked, errors


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("site", nargs="?", type=Path, default=Path("site"))
    parser.add_argument("--site-url")
    args = parser.parse_args()
    site_url = args.site_url
    if site_url is None:
        config = Path(__file__).resolve().parents[1] / "zensical.toml"
        site_url = tomllib.loads(config.read_text())["project"]["site_url"]
    checked, errors = check(args.site, site_url)
    for error in errors:
        print(error)
    print(
        f"Checked {checked} generated links and media references; {len(errors)} errors"
    )
    raise SystemExit(1 if errors else 0)


if __name__ == "__main__":
    main()
