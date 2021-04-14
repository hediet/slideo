#!/bin/bash

set -vex

# See https://cmichel.io/how-to-install-an-old-package-version-with-brew/. Needs to be compiled though.
#brew tap-new $USER/local-opencv
#brew extract --version=$OPENCV_VERSION opencv $USER/local-opencv
brew install opencv@4
