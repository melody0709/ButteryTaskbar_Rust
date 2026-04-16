fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = tauri_winres::WindowsResource::new();
        res.set_icon("assets/buttery-taskbar.ico");
        res.set("FileDescription", "Buttery Taskbar");
        res.set("ProductName", "Buttery Taskbar");
        res.set("InternalName", "buttery-taskbar");
        res.set("OriginalFilename", "buttery-taskbar.exe");
        res.set("CompanyName", "melody0709");
        res.set("ProductVersion", "2.5.1");
        res.set("FileVersion", "2.5.1");
        if let Err(e) = res.compile() {
            eprintln!("cargo:warning=failed to compile Windows resources: {e}");
        }
    }
}
