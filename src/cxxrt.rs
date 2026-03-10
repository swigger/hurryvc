// use directcpp::FutureValue;
#[repr(C)]
#[derive(Default, Clone)]
pub struct ProcessResult {
	pub code: i32,
	pub pid: u32, // the process ID of the new process.
	pub pcon: usize, // the pseudo handle of the new console.
	pub input_writer: usize, // the writing end of the pipe for the new console's stdin.
	pub output_reader: usize, // the read end of the pipe for the new console's stdout
	pub hprocess: usize, // the handle of the new process.
}

// __end_of_rust2h_header__

#[directcpp::bridge]
extern "C++" {
	pub fn cpp_main() -> i32;
	pub fn run_process_in_pty(cmdline:&str, width:u16, height:u16, work_dir:&str) -> ProcessResult;
	pub fn resize_process_in_pty(result: &ProcessResult, width:u16, height:u16) -> i32;
	pub fn destory_process(result: &ProcessResult);
}

#[directcpp::enable_msvc_debug]
struct UnusedDebugging{}
