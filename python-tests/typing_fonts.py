from pathlib import Path
from vestra import Text, list_system_fonts

families: list[str] = list_system_fonts()
default_text: Text = Text("Hello", position=(0.5, 0.5), rotation_degrees=12)
none_text: Text = Text("Hello", font=None, font_size=48)
family_text: Text = Text("Hello", font="Test Family", font_size=48)
file_text: Text = Text("Hello", font=Path("font.ttf"), font_size=48)
font: str | None = default_text.font
styled_text: Text = Text(
    "Hello",
    fallback_fonts=[Path("local.ttf"), "Fallback Family"],
    stroke_color="#102030",
    stroke_width=2,
    background_color="#10203080",
    background_padding=5,
)
fallback_fonts: tuple[str, ...] = styled_text.fallback_fonts
