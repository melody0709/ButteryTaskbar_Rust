fn main() {
    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/buttery-taskbar.ico");
        res.compile().expect("failed to compile Windows resources");
    }
}