fn main() {
    cc::Build::new().file("src/dlopen.c").compile("dlopen");
}
