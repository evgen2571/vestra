from vestra import Project
from vestra.audio import FilteredDelay, PlaybackSpeed


project = Project(size=(2, 2), fps=1, duration=1)
track = project.audio.track("music")
clip = track.add(123)
clip.effects.add("not an effect")
clip.set_gain_automation([object()])
track.effects.add(PlaybackSpeed(1))
clip.effects.add(PlaybackSpeed(1.10, link_pitch=1))
track.effects.add(FilteredDelay(delay_ms=125, repeats=1.5, mix=0.1, cutoff_hz=2300))
