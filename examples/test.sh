#!/usr/bin/env bash

PNG_EMBED=../target/debug/png_embed
[ ! -f "${PNG_EMBED}" ] && PNG_EMBED=../target/release/png_embed
[ ! -f "${PNG_EMBED}" ] && echo "Failed to find png_embed executable" && exit

echo "Encoding..."
for i in {1..7}; do
	echo "Writing dec-lo${i}.."
	$PNG_EMBED -l lo${i} -e embed.png input.png -o out-lo${i}.png
done

echo "Decoding..."
for i in {1..7}; do
	echo "Decoding out-lo${i} -> dec-lo${i}.."
	$PNG_EMBED -l lo${i} -d out-lo${i}.png -o dec-lo${i}.png
done

echo "Checksums:"
sha256sum embed.png dec-lo*.png # That's nuts!
