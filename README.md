# EPUB

Rust library for working with [epub] e-book files. This is a `no_std`
and `alloc` library. 

## Description

The library requires that the [epub] file be expanded into a directory
on the disk. Once it has been expanded, then metadata, navigation, and
content can be extracted.

The epub format is a zip file format with compressed files. The compression
algorithm, [microz],  uses 64k for the input and output decompression buffers. Those
buffers are allocated on the stack.

The library requires a fat filesystem to work, it is using [fatfs].
The epub file is expanded into a directory on the fat
filesystem.

## Example Disk Image

to mount this image
```
sudo mount -o ro,loop,offset=1048576 disk.img <mount path>
```

## Credits

* [fatfs](https://github.com/rafalh/rust-fatfs)
* [microz](https://github.com/Frommi/miniz_oxide)
* [RustyXML](https://github.com/Florob/RustyXML)

[epub]: https://www.w3.org/publishing/epub32/epub-spec.html

