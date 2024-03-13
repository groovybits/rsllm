#!/bin/bash
#
#
# Fonts from https://corefonts.sourceforge.net/

cabextract --lowercase --directory=cab-contents trebuc32.exe

cp -f cab-contents/trebuc.ttf TrebuchetMS.ttf
cp -f cab-contents/trebucbd.ttf TrebuchetMSBold.ttf
rm -rf cab-contents

