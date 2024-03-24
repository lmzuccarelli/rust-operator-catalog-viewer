## Overview

This is a simple POC that downloads a catalog operator index and has a simple terminal front end for viewing each operator metadata within the catalog. 

## POC 

This is still a WIP. I haven't completed all unit testing and there are bugs to sort out. 

I used a simple approach - Occam's razor

- A scientific and philosophical rule that entities should not be multiplied unnecessarily (KISS)
- Only RedHat's operator catalog index has been tested

## Usage

Clone this repo

Ensure that you have the correct permissions set in the $XDG_RUNTIME_DIR/containers/auth.json file

Execute the following to copy to local disk 

```bash
cd rust-catalog-operator-viewer

mkdir -p working-dir

chmod -R 777 working-dir (permissions need to be relaxed for untarred blobs)

cargo build 

# create an ImageSetConfig (this uses the example in this repo)
kind: ImageSetConfiguration
apiVersion: alpha1
mirror:
  operators:
  - catalog: "registry.redhat.io/redhat/redhat-operator-index:v4.15"

# build
make build

# download and untar the blobs
./target/debug/rust-operator-catalog-viewer --ui false --config imagesetconfig.yaml --loglevel debug --base-dir ./working-dir 

# copy the full dir from the previous step 
# execute the viewer
# in my instance the full path is ./working-dir/redhat-operator-index/v4.15/cache/071eb5/configs/
./target/debug/rust-operator-catalog-viewer --ui true --config imagesetconfig.yaml --loglevel debug --base-dir ./working-dir/redhat-operator-index/v4.15/cache/071eb5/configs/ 

```

## Unit Testing & Code coverage

Ensure grcov and  llvm tools-preview are installed

```
cargo install grcov 

rustup component add llvm-tools-preview

```

execute the tests

```
# add the -- --nocapture or --show-ouput flags to see println! statements
$ CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='cargo-test-%p-%m.profraw' cargo test

# for individual tests
$ CARGO_INCREMENTAL=0 RUSTFLAGS='-Cinstrument-coverage' LLVM_PROFILE_FILE='cargo-test-%p-%m.profraw' cargo test create_diff_tar_pass -- --show-output
```

check the code coverage

```
$ grcov . --binary-path ./target/debug/deps/ -s . -t html --branch --ignore-not-existing --ignore '../*' --ignore "/*" --ignore "src/main.rs" -o target/coverage/html

```
