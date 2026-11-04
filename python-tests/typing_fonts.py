from pathlib import Path
from vestra import Text, list_system_fonts

families: list[str] = list_system_fonts()
default_text: Text = Text("Hello", font_size=48)
none_text: Text = Text("Hello", font=None, font_size=48)
family_text: Text = Text("Hello", font="Test Family", font_size=48)
file_text: Text = Text("Hello", font=Path("font.ttf"), font_size=48)
font: str | None = default_text.font
