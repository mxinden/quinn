#[cfg(any(target_os = "macos", target_os = "ios"))]
fn main() {
    use std::env;
    use std::path::PathBuf;

    // Generate the bindings for Apple's private `recvmsg_x` from
    // https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/socket.h.
    let bindings = bindgen::Builder::default()
        .clang_arg("-DPRIVATE=1")
        .allowlist_function("recvmsg_x") // TODO: sendmsg_x
        .no_copy("iovec") // msghdr_x
        .header("src/bindings/socket.h")
        .generate()
        .expect("Unable to generate bindings");

    // Write the bindings to the $OUT_DIR/bindings.rs file.
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write bindings!");
}

#[cfg(not(any(target_os = "macos", target_os = "ios")))]
fn main() {}
