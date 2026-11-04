"""Frames to a video file through FFmpeg (on the PATH, or given)."""

import shutil
import subprocess


class VideoWriter:
    """Pipes RGBA frames to FFmpeg::

        with VideoWriter("out.mp4", 1280, 720, fps=30) as video:
            video.write(image)   # an fflame.Image, or width*height*4 RGBA bytes

    The default is H.264 at CRF 18 in yuv420p, which players expect; an
    odd size is padded to even, which yuv420p needs. `args` replaces the
    encoder arguments (everything after the input).
    """

    def __init__(self, path, width, height, fps=30, codec="libx264", crf=18, args=None, ffmpeg=None):
        exe = ffmpeg or shutil.which("ffmpeg")
        if not exe:
            raise FileNotFoundError("FFmpeg is not on the PATH (pass ffmpeg=...)")
        self.width, self.height = width, height
        if args is None:
            args = ["-c:v", codec, "-crf", str(crf), "-pix_fmt", "yuv420p"]
            if width % 2 or height % 2:
                args += ["-vf", "pad=ceil(iw/2)*2:ceil(ih/2)*2"]
        cmd = [exe, "-y", "-loglevel", "error", "-f", "rawvideo", "-pix_fmt", "rgba",
               "-s", f"{width}x{height}", "-r", str(fps), "-i", "-", *args, str(path)]
        self._proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stderr=subprocess.PIPE)
        self.frames = 0

    def write(self, frame):
        data = getattr(frame, "rgba", frame)
        if len(data) != self.width * self.height * 4:
            raise ValueError(f"a frame of {len(data)} bytes is not {self.width}x{self.height} RGBA")
        self._proc.stdin.write(data)
        self.frames += 1

    def close(self):
        if self._proc is None:
            return
        self._proc.stdin.close()
        err = self._proc.stderr.read().decode(errors="replace")
        code = self._proc.wait()
        self._proc = None
        if code != 0:
            raise RuntimeError(f"FFmpeg failed ({code}): {err.strip()}")

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()
