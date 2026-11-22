+++
title = "How To Render Newlines In Terminals"
date = 2026-11-22
authors = ["Tom Buckley-Houston"]
+++

Do you know how to output newlines in a terminal? Just output a `\n` right? If only it were that simple.
<!-- more -->

You may already know that there is a difference between Unix and Windows line endings. Unix ends lines with `\n` (aka carriage return, LF) and Windows ends lines with `\r\n` (aka line feed CR ie CRLF). You may also have experienced messy looking terminal output on occasions, like this:

![Screenshot of messy line endings](/assets/screenshots/ONLCR.png)

The reason for the messy appearance is quite simple: it's what happens when you just line feed and don't carriage return. If you look closely at the screenshot you might see that in the messy terminal I set `stty -onlcr` before calling `ls`. That `stty` command disables a feature of the kernel. Excuse me? The kernel?? Yes, the [kernel's TTY driver has a feature called Line Discipline](https://kernel-internals.org/tty/line-disciplines/) that amongst other responsibilities converts singular `\n`s to `\r\n`s.

But why the kernel? With all its magnificent duties, why does it burden itself with such a lowly task? Firstly we need to know a bit of computing history. Way back, when computers used to be the size of small houses, users accessed them via typewriter-like "terminals". They would print output onto actual paper and commonly moved their write head position via line feeds and carriage returns, indeed [this is where those names originally come from](https://www.youtube.com/watch?v=8cxJm3jcESE). So then when it came to storing text in computer memory, it seemed reasonable to append CR and LF characters at the end of each line. But seeing as digital bits aren't burdened by the physical realities of typewriters, it was actually possible for text files to just use LF to represent the end of a line, such that LF would trigger whatever movements the physical writer needed to start a new line. [This was first standardised by a Unix-like OS called Multics](https://en.wikipedia.org/wiki/Newline#History):

> The Multics operating system began development in 1964 and used LF alone as its newline. Multics used a device driver to translate this character to whatever sequence a printer needed (including extra padding characters), and the single byte was more convenient for programming

Reducing typing and memory was a very real concern in the 1960s. Of course nowadays line endings are automatically added in our editors and we have more than enough RAM for 2-character line endings, but the convention has been established and it doesn't cause any major issues. It's just surprising that modern terminal GUIs, with image protocols, GPU support, and so on, still rely on the kernel to correctly render new lines.

I'm building an entire new terminal architecture from scratch, which will remove all archaic features like the kernel TTY driver, escape codes, and so on. The working title is Tayland, because it's a paradigm shift as great as Wayland. [I'm doing it all live on Twitch, come say hi](https://www.twitch.tv/tom__bh).
