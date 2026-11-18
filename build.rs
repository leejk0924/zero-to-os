fn main() {
    // 링커 스크립트를 링크 인자로 넘겨야 이미지가 0x80000000부터 우리가 정한 배치로 만들어진다.
    // 절대 경로를 쓰는 이유: 링커가 실행되는 작업 디렉터리가 일정하지 않기 때문이다.
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    println!("cargo:rustc-link-arg=-T{dir}/kernel.ld");
    // 이 줄이 없으면 링커 스크립트만 바꿨을 때 cargo가 다시 링크하지 않는다.
    println!("cargo:rerun-if-changed=kernel.ld");
}
