#!/usr/bin/env sh
ROOT=$(git rev-parse --show-toplevel)
cd $ROOT

verify_bin=$(which solana-verify)
if [ "$?" != "0" ]; then
    echo "solana-verify not found. Please run: cargo install solana-verify."
    exit 1
fi

program_lib_name=$1
deployment=$2

if [ -z "$program_lib_name" ] || [ -z "$deployment" ]; then
    echo "Usage: $0 <program_lib_name> <deployment>"
    exit 1
fi

if [ "$deployment" = "mainnet" ]; then
    features="--features mainnet-beta,custom-heap"
elif [ "$deployment" = "devnet" ]; then
    features="--features devnet,custom-heap --no-default-features"
elif [ "$deployment" = "staging" ]; then
    features="--features staging,custom-heap --no-default-features"
elif [ "$deployment" = "stagingalt" ]; then
    features="--features stagingalt,custom-heap --no-default-features"
else
    echo "Error: Unknown deployment: $deployment"
    exit 1
fi

export CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS=true
export CARGO_PROFILE_RELEASE_LTO=fat
export CARGO_PROFILE_RELEASE_CODEGEN_UNITS=1
export CARGO_PROFILE_RELEASE_OPT_LEVEL=3
export CARGO_PROFILE_RELEASE_INCREMENTAL=false

cmd="sudo $verify_bin build --library-name $program_lib_name -- $features"
echo "Running: $cmd"
eval "$cmd"
