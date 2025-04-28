## Overview

This is a simple POC that downloads a catalog operator index and has a simple terminal front end for viewing each operator metadata within the catalog. 

## POC 

This is still a WIP. I haven't completed all unit testing and there are bugs to sort out. 

I used a simple approach - Occam's razor

- A scientific and philosophical rule that entities should not be multiplied unnecessarily (KISS)
- Only **RedHat's operator catalog index** has been tested

## Usage

This assumes you already have installed the rust binaries (https://www.rust-lang.org/tools/install)


Clone this repo

Ensure that you have the correct permissions set in the $XDG_RUNTIME_DIR/containers/auth.json file

You can download a pull secret from https://console.redhat.com/openshift/install/pull-secret and copy it to $XDG_RUNTIME_DIR/containers/auth.json

**NB** In the latest update the command line now has 2 subcommands i.e 
- Update
- View

Use the following command line to get help

```bash
./target/release/operator-catalog-viewer help
```

```bash
cd rust-catalog-operator-viewer

mkdir -p working-dir

chmod -R 755 working-dir (permissions need to be relaxed for untarred blobs)

# create an CatalogDownloadConfig (this uses the example in this repo)
# note as this only refers to operators , platform releases and additionalImages should not be included
kind: CatalogDownloadConfig
apiVersion: mirror.openshift/v3alpha1
mirror:
  operators:
  - catalog: "registry.redhat.io/redhat/redhat-operator-index:v4.15"

```
Building from source 

Execute the following command/s

**N.B.** Ensure all depenedencies are included

i.e for Fedora install the following (this will vary for different distros)

```
sudo dnf groupinstall "Development Tools"
```

Use the Makefile

```bash
# build
# this uses Rust build optimization (see Cargo.toml for more details)
# current binary is 3.1M
make build

# I used the common cache directory that I created previously with the customized version of bulk mirroring redhat images
# refer to the project https://github.com/lmzuccarelli/rust-image-mirror
# Download and untar the blobs
./target/release/operator-catalog-viewer --loglevel info update --config-file examples/catalog-download-config.yaml --working-dir ../rust-image-mirror/working-dir 

# use the full dir link (output from console) from the previous step 
# execute the viewer
# in my instance the full path is ./working-dir/redhat-operator-index/v4.15/cache/071eb5/configs/
./target/release/operator-catalog-viewer view --configs-dir ./working-dir/redhat-operator-index/v4.15/cache/071eb5/configs/ 

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

**NB** A makefile s included that simplifies the testing and code coverage 

```bash
make test && make coverage

```

Also note I have not done any unit tests for a TUI. Only the update section of the code has unit tests.
