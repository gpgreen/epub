## EPUB

Rust library for working with epub e-book files. This is a `no_std`
library. It does use `alloc`, the heap may be quite small. The
compression library needs a heap to expand blocks.

The library requires a fat filesystem to work, it is using `fatfs`.
The epub file is expanded into a directory on the fat
filesystem. The directory is hardcoded for now into the root directory
of the fat filesystem, and is intended for only one epub file.

All paths are constricted to 256 bytes in length.

## Example Disk Image

to mount this image
```
sudo mount -o ro,loop,offset=1048576 disk.img <mount path>
```
