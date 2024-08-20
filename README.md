# png_data -- Data as images

## png_data -- Data to PNG

![TeX Live english documentation](doc/texlive_en.png)

`png_data` encodes a file into a png image.

### Encoding
`png_data -l rgb8 -e file.pdf -o output.png -c "(.pdf) documentation"`
Where:
 * `rgb8` is the RGB layout with 8 bits per pixel
 * `file.pdf` is the file to store in the resulting image
 * `output.png` the resulting png image

**Available layouts**
 * `rgb[8|16]` RGB with 8 or 16 bits per channel
 * `rgba[8|16]` RGBA with 8 or 16 bits per channel (densest layout)
 * `g[1|2|4|8|16]` Grayscale with 1-16 bits per channel
 * `ga[1|2|4|8|16]` Grayscale Alpha with 1-16 bits per channel

### Decoding
`png_data -d output.png -o original.pdf`
Where:
 * `output.png` the encoded png image
 * `original.pdf` the resulting decoded file

### Getting header information
`png_data -z output.png`
 * `output.png` a `png_data` encoded image
This will display the header of the encoded file, as well as the comment.


## png_embed -- Embed files into mostly innocent PNG

![Contains an embed](doc/with_embed.png)
![The embed](doc/embed.png)

`png_embed` encodes a file into an existing png image making it possible to recover that file by passing the image around.

### Current algorithm:
 * `lo` Embeds data in the colors channels lowest bits.

See [examples/test.sh](examples/test.sh) for usage.

### Encoding an image
`png_embed -l lo2 -e embed.tar original.png -o output.png -c "(.tar) archive"`
Where:
 * `lo2` is the `Lo` algorithm using the 2 lowest bits
 * `embed.tar` the file to embed into the final image
 * `original.png` the original PNG file
 * `output.png` the resulting PNG file
 * `"(.tar) archive"` an optional comment

**Additional Options**
 * `-s|--seed TXT` Sets the random seed for determining the payload blocks. By default the random seed is "WIDTHxHEIGHT" where WIDTH and HEIGHT are the original image's dimensions.
 * `-n|--entropy` Fills unused payload blocks with random data that tries to match the payload's entropy. This feature is experimental and may not fully protect against entropy based steganography-detection. We highlihy recommend that the payload has maximal entropy, which can be achieved by compressing it.

### Decoding an image
`png_embed -l lo2 -d image.png -o embed.tar`
Where:
 * `lo2` is the `Lo` algorithm using the 2 lowest bits
 * `image.png` the PNG containing an embed
 * `embed.tar` the extracted embedded file

**Additional Options**
 * `-s|--seed TXT` Sets the random seed for determining the payload blocks. By default the random seed is "WIDTHxHEIGHT" where WIDTH and HEIGHT are the original image's dimensions.

### Getting header information
`png_embed -l lo2 -z output.png`
 * `lo2` is the `Lo` algorithm using the 2 lowest bits
 * `output.png` a `png_embed` encoded image
This will display the header of the encoded file, as well as the comment.

**Additional Options**
 * `-s|--seed TXT` Sets the random seed for determining the payload blocks. By default the random seed is "WIDTHxHEIGHT" where WIDTH and HEIGHT are the original image's dimensions.

# License

png_data is licensed under the GNU AGPL version 3 or later. See [LICENSE.md](LICENSE.md) for more information.
License for third-party dependencies can be accessed via `cargo license`
