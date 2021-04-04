# Slideo: Synchronize Slides With Video

This tool uses OpenCV to automatically synchronize slides with videos that show these slides.

After synchronization, you can click on a slide to play the video from this slide.

_This is tool is more a proof of concept rather than a polished product. Please reach out if you want to make a nice open source product out of it._

![](./docs/demo.gif)

## Installation

Go to the releases and download and extract the appropriate release.

### Windows Requirements

-   [Microsoft Visual C++ Redistributable für Visual Studio 2019](https://visualstudio.microsoft.com/de/downloads/#microsoft-visual-c-redistributable-for-visual-studio-2019)

### Linux, Mac Requirements

-   OpenCV 4.5.1
-   Poppler

## Usage

### Synchronize A Set of PDFs with a Set of Videos

Any given lecture slide can appear in any given video.
For performance and accuracy, keep the amount of pdf files small and prefer seperate invocations.
Usually, an entire lecture (<1000 slides) works well.

```sh
slideo lecture1.pdf lecture2.pdf video1.mp4 video2.mp4
```

_Warning: Takes about a tenth of the video duration!_

When you know that video1 does not contain any slides of lecture2, do this:

```sh
slideo lecture1.pdf video1.mp4 --non-interactive && slideo lecture2.pdf video2.mp4 --non-interactive
```

### View A Synchronized PDF

```
slideo lecture1.pdf
```

This will spawn a webserver on port 63944 and print an url that you can open in your favorite browser.

### Data Organization

SQLite is used to keep track of the slide/frame mapping.
Every file is identified by its hash, so moving files around is not problematic.

## TODO

-   Use wry to build a proper web GUI.
-   Use rustcv to get rid of the OpenCV dependency.
