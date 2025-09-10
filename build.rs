use std::path::Path;

fn main() {
    // 仅在 Windows 平台上配置图标
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        
        // 设置应用程序图标
        if Path::new("icon.ico").exists() {
            res.set_icon("icon.ico");
        }
        
        // 设置应用程序信息
        res.set("ProductName", "BeatCLI");
        res.set("ProductVersion", "0.1.0");
        res.set("FileDescription", "跨平台控制台音乐播放器");
        res.set("CompanyName", "BeatCLI Team");
        res.set("LegalCopyright", "Copyright (c) 2024");
        res.set("OriginalFilename", "BeatCLI.exe");
        
        // 编译资源
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resources: {}", e);
        }
    }
    
    // 告诉 Cargo 如果图标文件改变了就重新构建
    println!("cargo:rerun-if-changed=icon.ico");
    println!("cargo:rerun-if-changed=icon.svg");
}