# ensure_no_std

binary crate to ensure that epub library doesn't depend on std

Make sure using nightly
```
rustup override set nightly
```

crate should compile with the following
```
cargo rustc -- -C link-arg=-nostartfiles
```
