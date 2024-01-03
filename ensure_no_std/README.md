# ensure_no_std

binary crate to ensure that epub library doesn't depend on std. If it
doesn't compile, then std dependencies have been pulled in.

## Build
Make sure using nightly
```
rustup override set nightly
```

crate should compile with the following
```
cargo rustc -- -C link-arg=-nostartfiles
```

## CI
GitHub actions:
```
- name: Ensure that crate is no_std
  uses: actions-rs/cargo@v1
  with:
    command: rustc
    args: --manifest-path=ensure_no_std/Cargo.toml -- -C
  link-arg=-nostartfiles
```
