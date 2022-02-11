fn main() {
    cc::Build::new()
        .file("c_lib/localtime.c")
        // tm_zone has undocumented lifetime so better turn it off
        .define("NO_TM_ZONE", None)
        .define("STD_INSPIRED", None)
        .warnings(std::env::var_os("RL_LOCALTIME_WARN").is_some())
        .compile("rllocaltime");
}
