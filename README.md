# Slideo: Synchronize Slides With Video

This tool matches video frames against PDF pages by using computer vision.
It also ships a web app in which you can click on a PDF page to play the video from the first frame showing the page.
Its primary use-case is to quickly play a recorded lecture from a given slide.

Works best if the PDF page in the video is captured through screen recording and video is 1080p, but it might work in other scenarios too (rotation, shifting, scaling and obstruction is supported).

See [Background](./background.md) for how the matching algorithm works.

_This is tool is more a proof of concept rather than a polished product. Please reach out if you want to make a nice open source product out of it._

![](./docs/demo.gif)

## Installation

See [Releases](https://github.com/hediet/slideo/releases).
An installer is not yet provided.

### Windows Requirements

-   [Microsoft Visual C++ Redistributable für Visual Studio 2019](https://visualstudio.microsoft.com/de/downloads/#microsoft-visual-c-redistributable-for-visual-studio-2019)

### Linux, Mac Requirements

(untested)

-   OpenCV 4.5.1
-   Poppler

## Usage

### Synchronize A Set of PDFs with a Set of Videos

Any given lecture slide can appear in any given video.
For performance and accuracy, keep the amount of pdf files small and prefer seperate invocations.
Synchronizing an entire lecture (<1000 slides) should work well though.

```sh
slideo lecture1.pdf lecture2.pdf video1.mp4 video2.mp4
```

When you know that video1 does not contain any slides of lecture2, you can do this to improve accuracy:

```sh
slideo lecture1.pdf video1.mp4 --non-interactive && slideo lecture2.pdf video2.mp4 --non-interactive
```

### View A Synchronized PDF

```
slideo lecture1.pdf
```

This will spawn a webserver on port 63944 and print an url that you can open in your favorite browser.

## TODO

-   Use wry to build a proper web GUI.
-   Use rustcv to get rid of the OpenCV dependency.
