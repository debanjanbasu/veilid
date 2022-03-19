#!/bin/bash
SCRIPTDIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" >/dev/null 2>&1 && pwd )"

pushd $SCRIPTDIR &> /dev/null

if [[ "$1" == "debug" ]]; then  
    OUTPUTDIR=../target/wasm32-unknown-unknown/debug/pkg
    INPUTDIR=../target/wasm32-unknown-unknown/debug

    RUSTFLAGS="-O -g" cargo build --target wasm32-unknown-unknown
    mkdir -p $OUTPUTDIR
    wasm-bindgen --out-dir $OUTPUTDIR --target web --no-typescript --keep-debug --debug $INPUTDIR/veilid_wasm.wasm
    ./wasm-sourcemap.py $OUTPUTDIR/veilid_wasm_bg.wasm -o $OUTPUTDIR/veilid_wasm_bg.wasm.map --dwarfdump `which llvm-dwarfdump`
    wasm-strip $OUTPUTDIR/veilid_wasm_bg.wasm
else
    OUTPUTDIR=../target/wasm32-unknown-unknown/release/pkg
    INPUTDIR=../target/wasm32-unknown-unknown/release

    cargo build --target wasm32-unknown-unknown --release
    mkdir -p $OUTPUTDIR
    wasm-bindgen --out-dir $OUTPUTDIR --target web --no-typescript $INPUTDIR/veilid_wasm.wasm
    wasm-strip $OUTPUTDIR/veilid_wasm_bg.wasm
fi

popd &> /dev/null