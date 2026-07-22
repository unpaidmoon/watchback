"""Convert chroma-key achievement token renders into packaged RGBA assets.

Expected input names are ``tier-1-key.png`` through ``tier-3-key.png``.
The script delegates matte extraction to Codex's imagegen helper, then centers
each result on a transparent 512 x 512 canvas and validates the alpha channel.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

from PIL import Image


TIERS = (1, 2, 3)
CANVAS_SIZE = 512
SUBJECT_SIZE = 480


def codex_home() -> Path:
    configured = os.environ.get("CODEX_HOME")
    if configured:
        return Path(configured)

    user_profile = os.environ.get("USERPROFILE")
    if not user_profile:
        raise RuntimeError("缺少 CODEX_HOME 或 USERPROFILE，无法定位透明底处理工具。")
    return Path(user_profile) / ".codex"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="处理成就徽章洋红色键背景并输出透明 PNG")
    parser.add_argument("--group", required=True, help="资源组名，例如 win-rate")
    parser.add_argument("--input-dir", type=Path, required=True, help="包含 tier-N-key.png 的目录")
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=Path("assets/achievements/token-style"),
        help="正式资源输出目录",
    )
    return parser.parse_args()


def extract_alpha(helper: Path, source: Path, target: Path) -> None:
    subprocess.run(
        [
            sys.executable,
            str(helper),
            "--input",
            str(source),
            "--out",
            str(target),
            "--auto-key",
            "border",
            "--soft-matte",
            "--transparent-threshold",
            "12",
            "--opaque-threshold",
            "220",
            "--despill",
            "--force",
        ],
        check=True,
    )


def package_asset(source: Path, target: Path) -> None:
    with Image.open(source) as opened:
        badge = opened.convert("RGBA")
        badge.thumbnail((SUBJECT_SIZE, SUBJECT_SIZE), Image.Resampling.LANCZOS)
        canvas = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
        offset = ((CANVAS_SIZE - badge.width) // 2, (CANVAS_SIZE - badge.height) // 2)
        canvas.alpha_composite(badge, offset)
        canvas.save(target, optimize=True)


def validate_asset(target: Path) -> None:
    with Image.open(target) as opened:
        rgba = opened.convert("RGBA")
        alpha = rgba.getchannel("A")
        if rgba.size != (CANVAS_SIZE, CANVAS_SIZE):
            raise RuntimeError(f"{target} 尺寸不是 {CANVAS_SIZE}x{CANVAS_SIZE}")
        if alpha.getextrema() != (0, 255):
            raise RuntimeError(f"{target} 缺少完整透明与不透明像素")
        if any(rgba.getpixel(point)[3] != 0 for point in ((0, 0), (511, 0), (0, 511), (511, 511))):
            raise RuntimeError(f"{target} 四角未完全透明")


def main() -> None:
    args = parse_args()
    helper = codex_home() / "skills" / ".system" / "imagegen" / "scripts" / "remove_chroma_key.py"
    if not helper.is_file():
        raise RuntimeError(f"找不到透明底处理工具：{helper}")

    args.output_dir.mkdir(parents=True, exist_ok=True)
    for tier in TIERS:
        keyed = args.input_dir / f"tier-{tier}-key.png"
        alpha = args.input_dir / f"tier-{tier}-alpha.png"
        output = args.output_dir / f"{args.group}-token-tier-{tier}.png"
        if not keyed.is_file():
            raise RuntimeError(f"缺少输入文件：{keyed}")
        extract_alpha(helper, keyed, alpha)
        package_asset(alpha, output)
        validate_asset(output)
        print(f"已生成 {output}")


if __name__ == "__main__":
    main()
