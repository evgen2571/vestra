from vestra import Project
from vestra.audio import PlaybackSpeed


project = Project(size=(2, 2), fps=1, duration=1)
track = project.audio.track("music")
clip = track.add(123)
clip.effects.add("not an effect")
clip.set_gain_automation([object()])
track.effects.add(PlaybackSpeed(1))
