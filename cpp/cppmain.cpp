#include "pch.h"
#include "cxxrt.h"
#include "ptyrun.h"

#ifdef _WIN32

int cpp_main() {
	CPseudoConsoleSession::init_console();
	// setlocale(LC_ALL, "en_US.UTF-8");
	WSADATA wsd;
	WSAStartup(0x202, &wsd);
#ifdef _DEBUG
	auto vsdir = getenv("VisualStudioDir");
	static int waited = 0;
	if (vsdir && *vsdir && waited == 0) {
		waited = 1;
		// if we're in debug mode and started from visual studio, and not in unit test mode,
		// show the "Press any key to continue" message
		wchar_t cmdline[400];
		GetModuleFileNameW(0, cmdline, _countof(cmdline));
		auto p = wcsrchr(cmdline, L'\\') + 1;
		if (wcsicmp(p, L"testhost.exe") != 0) {
			swprintf(cmdline, _countof(cmdline), L"waitpid -T 60 %d", GetCurrentProcessId());
			STARTUPINFO sinfo = { sizeof(sinfo) };
			PROCESS_INFORMATION pinfo{};
			if (CreateProcessW(nullptr, cmdline, nullptr, nullptr, FALSE, 0, nullptr, nullptr, &sinfo, &pinfo)) {
				WaitForSingleObject(pinfo.hProcess, 500); //wait for startup.
				CloseHandle(pinfo.hProcess);
				CloseHandle(pinfo.hThread);
			}
		}
	}
#endif
	return 0;
}

ProcessResult run_process_in_pty(char const* cmdline, unsigned __int64 cmdline_len,
	unsigned short width, unsigned short height, 
	char const* work_dir, unsigned __int64 work_dir_len)
{
	string cmd(cmdline, cmdline_len);
	string cwd;
	if (work_dir && work_dir_len > 0) {
		cwd.assign(work_dir, work_dir_len);
	}
	CPseudoConsoleSession session;
	CPseudoConsoleSession::Config cfg{};
	cfg.cmdline = cmd.c_str();
	cfg.width = width;
	cfg.height = height;
	cfg.workDir = cwd.empty() ? nullptr : cwd.c_str();
	PROCESS_INFORMATION pinfo{};
	ProcessResult res{};
	res.code = session.Start(cfg, pinfo);
	if (res.code == 0) {
		CloseHandle(pinfo.hThread);
		res.pid = pinfo.dwProcessId;
		res.pcon = (size_t)session.m_hPcon;
		res.input_writer = (size_t)session.m_hInputWrite;
		res.output_reader = (size_t)session.m_hOutputRead;
		res.hprocess = (size_t)pinfo.hProcess;

		session.m_hInputWrite = nullptr;
		session.m_hOutputRead = nullptr;
		session.m_hPcon = nullptr;
	}
	return res;
}

int resize_process_in_pty(struct ProcessResult const& pr, unsigned short width, unsigned short height) {
	if (!pr.pcon) {
		return -1;
	}
	COORD size{ (SHORT)width, (SHORT)height };
	return SUCCEEDED(ConptyResizePseudoConsole((HPCON)pr.pcon, size)) ? 0 : -1;
}

void destory_process(struct ProcessResult const& pr) {
	if (pr.hprocess) {
		CloseHandle((HANDLE)pr.hprocess);
	}
	if (pr.input_writer) {
		CloseHandle((HANDLE)pr.input_writer);
	}
	if (pr.output_reader) {
		CloseHandle((HANDLE)pr.output_reader);
	}
	if (pr.pcon) {
		ClosePseudoConsole((HPCON)pr.pcon);
	}
}

#else // _WIN32

int __argc = 0;
char** __argv = NULL;
__attribute__((constructor)) int my_entry(int argc, char** argv)
{
	__argc = argc;
	__argv = argv;
	return 0;
}

int cpp_main() {
	signal(SIGPIPE, SIG_IGN);
	return 0;
}

#endif

// enable some struct to be return-able in C++/Rust FFI boundary
// this is not need for foundamental types like int(i32), size_t(usize) etc.
// also not needed if the type is just used as a reference in argument, like HPCON* or PROCESS_INFORMATION*.
// this funcion is not used itself, but it forces the compiler to emit some code for these types.
void unused_function() {
	ffi::enable_class<RustString>();
	ffi::enable_class<RustVec<RustString>>();
	ffi::enable_class<ProcessResult>();
}
