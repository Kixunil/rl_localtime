fn main() {
    cc::Build::new()
        .file("c_lib/localtime.c")
        .warnings(std::env::var_os("RL_LOCALTIME_WARN").is_some())
        .compile("rllocaltime");
}
