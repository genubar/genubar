#!/usr/bin/env python3
# ==GenubarScript==
# @id          example_patch_script
# @name        Example Reversible Patch Script
# @kind        patch_script
# @version     1.0.0
# @author      genubar
# @description Template showing how to write a custom patch script with apply() and revert() methods.
# ==/GenubarScript==

"""
HOW TO USE THIS PATCH SCRIPT IN A PATCH (.toml):
1. Keep the `# ==GenubarScript== ... # ==/GenubarScript==` header at the top.
2. In your patch TOML file (`~/.config/genubar/patches/my_patch.toml`), add a step:
   ```toml
   [[steps]]
   type = "python_script"
   script = "scripts/example_patch_script.py"
   args = { target_file = "~/.config/my_app.conf" }
   ```
3. Implement TWO functions in this file:
    - `apply(ctx: dict) -> str | None`:
    Performs the patch. Whatever string (or JSON string) you return is saved into
    `~/.local/share/genubar/patch_ledger.json` as `original_value`!
    Raise an Exception if anything fails (Genubar will automatically roll back previous steps).

    - `revert(ctx: dict, saved_state: str | None) -> None`:
    Restores the original state using `saved_state` from `patch_ledger.json` (or cleans up if `saved_state` is `None`).
"""

from pathlib import Path

def apply(ctx: dict) -> str | None:
    """
    Called when the user clicks 'Apply Patch'.ctx contains:
    - ctx["patch_id"]: ID of the patch being applied
    - ctx["home"]: User's $HOME directory path
    - ctx["args"]: Dictionary of custom args passed from the patch TOML step
    Returns the previous state string to store in patch_ledger.json.
    """
    args = ctx.get("args", {})
    raw_path = args.get("target_file", "~/.cache/genubar/test_patch_demo.txt")
    target = Path(raw_path).expanduser()

    target.parent.mkdir(parents=True, exist_ok=True)
    previous_content = target.read_text(encoding="utf-8") if target.exists() else None
    target.write_text("# Patched by Genubar Python Script\nENABLED=true\n", encoding="utf-8")

    return previous_content

def revert(ctx: dict, saved_state: str | None) -> None:
    """
    Called when the user clicks 'Revert Patch' (or during automatic rollback).saved_state is the exact string returned by apply(ctx) (or None if the file did not exist).
    """
    args = ctx.get("args", {})
    raw_path = args.get("target_file", "~/.cache/genubar/test_patch_demo.txt")
    target = Path(raw_path).expanduser()

    if saved_state is not None:
        target.write_text(saved_state, encoding="utf-8")
    elif target.exists():
        target.unlink()
