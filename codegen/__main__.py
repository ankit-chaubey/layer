"""
Run the code generator:

    python -m layer.codegen

This regenerates ``layer/generated/`` from the TL schemas in ``layer/tl/``.
"""

from pathlib import Path
from .generator import generate, Config

def main() -> None:
    root = Path(__file__).parent.parent
    tl_dir = root / 'tl'
    output_dir = root / 'generated'

    print(f"🔧 layer code generator")
    print(f"   TL schemas : {tl_dir}")
    print(f"   Output     : {output_dir}")
    print()

    generate(
        tl_dir=tl_dir,
        output_dir=output_dir,
        config=Config(include_api=True, include_mtproto=False),
    )
    print()
    print("✅ Done! Generated files are ready in layer/generated/")

if __name__ == '__main__':
    main()
