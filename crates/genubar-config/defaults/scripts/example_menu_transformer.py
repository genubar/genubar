#!/usr/bin/env python3
# ==GenubarScript==
# @id          example_menu_transformer
# @name        Example Menu Transformer
# @kind        menu_transformer
# @version     1.0.0
# @author      genubar
# @description Template showing how to dynamically modify or add menu items in Python.
# ==/GenubarScript==

"""
HOW TO USE THIS SCRIPT IN GENUBAR:
1. Keep the `# ==GenubarScript== ... # ==/GenubarScript==` header at the top (files without it are ignored).
2. Reference this script in:
   - `~/.config/genubar/profiles/<your_profile>.toml`: `python_script = "scripts/example_menu_transformer.py"`
   - OR `desktop_menu.toml` / `app_menu.toml` with `mode = "python_script"`
3. Genubar passes a JSON object via STDIN with 3 keys:
   - `payload["window"]`: `{"pid": int, "xid": int, "title": str, "app_id": str, ...}`
   - `payload["tier"]`: `"gtk_native" | "gtk4_hook" | "canonical_dbus" | "atspi" | "desktop_idle"`
   - `payload["root"]`: The root `MenuNode` dictionary containing `"children": [...]` (top-level columns).
4. Print the modified `payload["root"]` as JSON to STDOUT.
"""

import json
import sys


def make_action_item(item_id: int, label: str, action: str, icon: str | None = None, shortcut: list[str] | None = None) -> dict:
    """Helper that builds a standard clickable MenuNode dictionary."""
    return {
        "id": item_id,
        "label": label,
        "enabled": True,
        "visible": True,
        "item_type": "Standard",
        "shortcut": shortcut,
        "action": action,
        "icon": icon,
        "children": [],
    }


def make_submenu(item_id: int, label: str, children: list[dict] | None = None) -> dict:
    """Helper that builds a top-level column or nested submenu MenuNode dictionary."""
    return {
        "id": item_id,
        "label": label,
        "enabled": True,
        "visible": True,
        "item_type": "Submenu",
        "shortcut": None,
        "action": None,
        "icon": None,
        "children": children if children is not None else [],
    }


def make_separator(item_id: int = -950) -> dict:
    """Helper that builds a horizontal separator MenuNode."""
    return {
        "id": item_id,
        "label": "",
        "enabled": True,
        "visible": True,
        "item_type": "Separator",
        "shortcut": None,
        "action": None,
        "icon": None,
        "children": [],
    }


def transform_menu(window: dict, tier: str, root: dict) -> dict:
    columns = root.setdefault("children", [])
    pid = window.get("pid", 0)

    # Example: Route a diagnostic "Copy PID" item into the `{app_name}` column.
    # `app_menu.toml` merges any column named `{app_name}` (or the app name / "App" / "Application")
    # into the leading application column above system items like Genubar Settings and Quit.
    if pid > 0:
        app_col = next(
            (c for c in columns if c.get("label", "").strip().lower() == "{app_name}"),
            None,
        )
        if app_col is None:
            app_col = make_submenu(-900, "{app_name}", [])
            columns.insert(0, app_col)

        app_items = app_col.setdefault("children", [])
        if app_items:
            app_items.append(make_separator(-901))
        app_items.append(
            make_action_item(
                item_id=-902,
                label=f"Copy PID ({pid})",
                action=f"copy:{pid}",
                icon="edit-copy-symbolic",
            )
        )

    return root


def main() -> None:
    payload = json.load(sys.stdin)
    window = payload.get("window", {})
    tier = payload.get("tier", "")
    root = payload.get("root", {})

    updated_root = transform_menu(window, tier, root)
    json.dump(updated_root, sys.stdout)


if __name__ == "__main__":
    main()