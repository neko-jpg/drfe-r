#!/bin/bash
. ~/.cargo/env
cd '/mnt/c/dev/network test'
cargo test --test property_tests
