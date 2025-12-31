#!/bin/bash
source ~/.cargo/env
cd '/mnt/c/dev/network test'
./target/release/simulator -n 100 --tests 500 --ttl 200 > /mnt/c/dev/network\ test/sim_tree_result2.md 2>&1
