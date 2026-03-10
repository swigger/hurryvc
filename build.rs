use build_vcxproj;

fn main() {
	println!("cargo:rerun-if-changed=hurryvc-ui/src");
	println!("cargo:rerun-if-changed=hurryvc-ui/public");
	println!("cargo:rerun-if-changed=hurryvc-ui/index.html");
	println!("cargo:rerun-if-changed=hurryvc-ui/package.json");
	println!("cargo:rerun-if-changed=hurryvc-ui/package-lock.json");
	println!("cargo:rerun-if-changed=hurryvc-ui/vite.config.ts");

	if std::env::var("PROFILE").as_deref() == Ok("release") {
		build_frontend();
	}

	println!("cargo::rustc-link-search=libs");
	let headers = ["cpp/*.h"];
	let sources = ["cpp/*.cpp"];
	let res_path = std::env::var("DEP_DIRECTCPP_RES_MPATH").unwrap();
	let incdirs = [res_path.as_str()]; // = ["some-dir"]
	if build_vcxproj::need_build("cpp/cxxrt.h", ["src/cxxrt.rs"]) {
		let python = if cfg!(windows) { "python" } else { "python3" };
		let rust2h = std::path::Path::new(&res_path).join("../tools/rust2h.py");
		let status = std::process::Command::new(python)
			.args([rust2h.to_str().unwrap(), "-o", "cpp/cxxrt.h", "src/cxxrt.rs"])
			.status()
			.expect("failed to run rust2h.py");
		assert!(status.success(), "rust2h.py failed");
	}
	let opt = build_vcxproj::sample_builder::BuildOptions::empty();
	build_vcxproj::sample_builder::build("hurryvc_lib", &headers, &sources, &incdirs, opt, |cxxb|{
		let _ = cxxb; // cxxb.flag("-Isrc")
		build_vcxproj::compile_rc("cpp/resource.rc");
	});
}

fn build_frontend() {
	let npm = if cfg!(windows) { "npm.cmd" } else { "npm" };
	let status = std::process::Command::new(npm)
		.args(["run", "build"])
		.current_dir("hurryvc-ui")
		.status()
		.expect("failed to run npm build for hurryvc-ui");
	assert!(status.success(), "frontend build failed");
}
