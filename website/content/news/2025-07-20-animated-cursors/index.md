+++
title = "Animated Cursors"
date = 2025-07-20
authors = ["Tom Buckley-Houston"]
[extra]
screenshot = "/assets/screenshots/cursor_blaze_large.webm"
description = "Animated cursors for all terminals"
+++

Tattoy now supports animated cursors. It uses the same format as Ghostty, therefore rendering the cursor using custom shaders.
<!-- more -->

[Here are some popular Ghostty cursors](https://github.com/KroneCorylus/ghostty-shader-playground/tree/main/shaders), that you can
use out-of-the-box with Tattoy.

Even though Tattoy supports Ghostty cursors its rendering is quite different. Ghostty renders the cursor using actual pixels
whereas Tattoy renders using UTF8 text-based "pixels", namely "▀" and "▄". This means that Tattoy cursors sometimes miss out on
the subtleties of Ghostty cursors, but of course the pixelated effect might also be pleasing to some.

Because Tattoy already has a shader based framework, it only took a couple of hours to get the first Ghostty shader working in Tattoy.
But it took at least another week to iron everything out. One of the hardest issues was supporting transparency for the
antialiased edges of cursor trails. Ghostty shaders expect to be able to sample the underlying pixels of the actual terminal, amongst
other things it's this sampling that allows for smooth antialiased blending. Tattoy however is purely text based and so can't do things
like get the individual pixels of font glyphs. But it does know the true colour values of text and so can create a crude "pixelised"
version of the terminal and upload that as an image buffer to the GPU. However, whilst this fixed the antialiasing, it also meant the
pixelised version of the terminal was included in the cursor image data. To fix this I added a simple post-processing step that
compares the Terminal pixels uploaded to the GPU with the final rendered cursor pixels. The difference between these 2 images is what
finally gets rendered to the user's terminal.

It seems to work pretty well. There can be a bit of lag on larger terminals. There are still lots of more general performance 
improvements that should help with that. But also I wonder if that could be remedied by Tattoy taking over all cursor rendering from
the host terminal emulator. Currently both the animated cursor and host cursor are rendered together, hence the difference in latency.

So I think this a good first attempt. Let me know how it goes for you.
