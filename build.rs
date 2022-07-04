use std::{process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/shaders/");
    Command::new("glslc").args(["-o", "assets/shaders/vert.spv", "assets/shaders/shader.vert"]).status().unwrap();
    Command::new("glslc").args(["-o", "assets/shaders/frag.spv", "assets/shaders/shader.frag"]).status().unwrap();
}