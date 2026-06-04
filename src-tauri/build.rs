// 默认调用 tauri_build::build() 编译所有资源。
// 在受限沙盒/容器中编译时，可设置 TAURI_SKIP_WIN_RESOURCE=1 跳过 tauri-build
// （仅用于语法/类型检查，不生成可执行文件）
fn main() {
    if std::env::var("TAURI_SKIP_WIN_RESOURCE").is_ok() {
        println!("cargo:warning=TAURI_SKIP_WIN_RESOURCE=1: skipping tauri-build (for type-check only).");
        println!("cargo:rerun-if-changed=build.rs");
        return;
    }
    tauri_build::build()
}
