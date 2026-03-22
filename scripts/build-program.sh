#!/usr/bin/env sh
ROOT=$(git rev-parse --show-toplevel)
cd $ROOT

program_lib_name=$1
cluster=$2

if [ -z "$program_lib_name" ] || [ -z "$cluster" ]; then
    echo "Usage: $0 <program_lib_name> <cluster>"
    exit 1
fi

if [ "$cluster" = "mainnet" ]; then
    features="--features mainnet-beta,custom-heap"
elif [ "$cluster" = "devnet" ]; then
    features="--features devnet,custom-heap --no-default-features"
elif [ "$cluster" = "staging" ]; then
    features="--features staging,custom-heap --no-default-features"
else
    echo "Error: Unknown cluster: $cluster"
    exit 1
fi

cmd="anchor build -p $program_lib_name -- $features"
echo "Running: $cmd"
eval "$cmd"
