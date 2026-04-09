fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/buttery-taskbar.ico");
        res.set("FileDescription", "Buttery Taskbar");
        res.set("ProductName", "Buttery Taskbar");
        res.set("InternalName", "buttery-taskbar");
        res.set("OriginalFilename", "buttery-taskbar.exe");
        res.set("CompanyName", "melody0709");
        res.compile().expect("failed to compile Windows resources");
    }
}