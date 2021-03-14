## EPUB

Rust library for working with epub e-book files. This is a `no_std`
and `alloc` library. The epub format has compressed files. The compression
algorithm uses 64k for the input and output decompression buffers. Those
buffers are allocated on the stack.

The library requires a fat filesystem to work, it is using `fatfs`.
The epub file is expanded into a directory on the fat
filesystem.

## Example Disk Image

to mount this image
```
sudo mount -o ro,loop,offset=1048576 disk.img <mount path>
```
