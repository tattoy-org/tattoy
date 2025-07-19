# Tattoy Website

The source files for <https://tattoy.sh>.

## Zola

The Tattoy website is built using the Zola static site engine.

To develop on the website:

1. [Install Zola](https://www.getzola.org/documentation/getting-started/installation/).
2. Clone the Tattoy git repo and enter the website directory:
   1. `git clone https://github.com/tombh/tattoy.git`
   2. `cd website`
3. Start the Zola server with `./ctl.sh serve`.

A local server should start and you should be able to access a local version of the website from [http://127.0.0.1:1111](http://127.0.0.1:1111).

## Making the gifs
* I record in Hyprland because my WM doesn't support capturing individual windows.
* Remember to resize the OBS canvas size to match the size of the terminal window.
* Convert to animated `.webm` with something like:
  `ffmpeg -y -i input.mkv -ss 00:00:05 -to 00:00:36 -vf "scale=500:-1" -r 10 -an -c:v libvpx-vp9 -quality 50 -loop 0 output.webm`

## Acknowledgements
* The code for this website was originally base on the [Bevy website repo](https://github.com/bevyengine/bevy-website).
* The logo is by [Sam Foster](https://cmang.org)
