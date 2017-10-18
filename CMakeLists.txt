cmake_minimum_required(VERSION 3.8)
project(BikeChain)

set(CMAKE_CXX_STANDARD 11)

set(SOURCE_FILES
        src/main.rs
        target/debug/BikeChain.dSYM/Contents/Resources/DWARF/BikeChain
        target/debug/BikeChain.dSYM/Contents/Info.plist
        target/debug/bikechain.d
        Cargo.lock
        Cargo.toml)

add_executable(BikeChain ${SOURCE_FILES})