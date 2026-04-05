extern crate winres;

fn main() {
	println!("cargo:rustc-link-arg=/STACK:10485760");
	if cfg!(target_os = "windows") {
		winres::WindowsResource::new().compile().unwrap()
	}
}
