# png_data -- Data as images

## png_embed -- Embed files into mostly innocent PNG

![Contains an embed](doc/with_embed.png)
![The embed](doc/embed.png)

### Current algorithm:
 * `lo` Embeds data in the colors channels lowest bits.

See [examples/test.sh](examples/test.sh) for usage.

### Encoding an image
`png_embed -l lo2 -e embed.tar original.png -o output.png`
Where:
 * `lo2` is the `Lo` algorithm using the 2 lowests bits
 * `embed.tar` the file to embed into the final image
 * `original.png` the original PNG file
 * `output.png` the resulting PNG file

### Decoding an image
`png_embed -l lo2 -d image.png -o embed.tar`
Where:
 * `lo2` is the `Lo` algorithm using the 2 lowests bits
 * `image.png` the PNG containing an embed
 * `embed.tar` the extracted embedded file

# License

NML is licensed under the GNU AGPL version 3 or later. See [LICENSE.md](LICENSE.md) for more information.
License for third-party dependencies can be accessed via `cargo license`
