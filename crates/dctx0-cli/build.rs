fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Match the usual Linux main-thread stack for unoptimized verification
        // command futures. The MSVC executable otherwise reserves only 1 MiB.
        println!("cargo:rustc-link-arg-bin=dctx0=/STACK:8388608");
    }
}
