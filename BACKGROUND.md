# Background

## Matching Algorithm

1. Extract every PDF page to PNG with `pdftocairo` from poppler.
2. Compute key points and their descriptors for every exported PNG file with the ORB feature extractor.
   ORB computes a list of tuples ((x, y), descriptor).
3. Put all the descriptors into a FLANN data structure (FLANN is a library for performing fast approximate nearest neighbor searches in high dimensional spaces), remember the slide and position of the descriptor.
4. Look at every `5*fps`-th frame of all videos:
    1. Skip if the frame did not change much (use norm2 to compute similarity).
    2. Compute key points and their descriptors of the frame (about 200-700).
    3. For each key point, find the best matching descriptors in the FLANN data structure with a tolerance of 5%, but at most 30.
    4. Group these matches by slide and only consider the top 40 slides with most matches.
    5. For every slide:
        1. Try to find a subset of matches that describe an affine transformation and compute that transformation.
           (OpenCV has a method for this - for rust cv this can be implemented with RANSAC + linear regression)
        2. Apply the inverse of the transformation to the frame and compute the similarity (norm2).
    6. Ignore slides with too few matches.
    7. Associate the frame with the slide that has the best similarity.
5. Remove consecutive matches with the same slide.

Do as much of this in parallel. Sadly, OpenCVs FLANN and ORB implementation are not thread-safe, so create and maintain one of them per thread.

Thanks to [phiresky](https://github.com/phiresky) who helped me prototyping a [Python PoC](https://github.com/phiresky/match-slides-to-recording).

## Data Organization

SQLite is used to keep track of the slide/frame mapping.
Every file is identified by its hash, so moving files around does not invalidate the mapping.
PDF pages are extracted into a temporary folder.

## Used Technologies

-   Rust, rayon and indicatif
-   OpenCV (with FFMPEG to decode videos)
-   SQLite (with sqlx for checked SQL queries)
-   actix-web to host the embedded viewer
-   TypeScript, Mobx, React, Video.js and Pdf.js for the viewer ([I created my own library to bundle the pdfjs viewer](https://github.com/hediet/pdf.js-viewer))

actix-web was the only web-framework I could find with direct support for video streaming/seeking of files with a custom url to file location resolver.

I used SQLite to make sure no accidental cache-corruption can happen, even if multiple instances of slideo are running.

OpenCV has tons of documentation and is production ready.
However, it is horrible to use from rust.
[rust-cv](https://github.com/rust-cv) seems to be a better fit for rust, however,
many features are still missing (such as the ORB feature extractor) and it does not seem to be production ready yet.
I'm open to migrate to rust-cv though and think it has a lot more potential than OpenCV.

## Building Instructions

See [`ci.yml`](./.github/workflows/ci.yml) for how the CI builds the project:

-   Install OpenCV
-   Install Rust, NodeJS and yarn
-   Run `yarn install` in the [webview](./webview) directory.
-   Run `yarn build` in the [webview](./webview) directory.
-   Run `cargo run` in the root directory.
