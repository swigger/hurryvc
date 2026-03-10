#include "pch.h"
#include "ptyrun.h"

#ifdef _WIN32
#ifdef _DEBUG
#pragma comment(lib, "conptylib-dbg.lib")
#else
#pragma comment(lib, "conptylib-rls.lib")
#endif

namespace {
	inline std::wstring a2w(const char* ptr, intptr_t len = -1, int codepage = CP_UTF8)
	{
		std::wstring con2;
		if (len < 0) len = strlen(ptr);
		if (len == 0) return con2;
		con2.resize(len + 3);
		int nr = MultiByteToWideChar(codepage, 0, ptr, (int)len, &con2[0], (int)(len + 3));
		con2.resize(nr > 0 ? nr : 0);
		return con2;
	}

	inline string w2a(const wchar_t* ws, intptr_t len = -1, int* has_err = 0, int codepage = CP_UTF8)
	{
		BOOL used_default = FALSE;
		string sf;
		if (len < 0) len = ws ? (int)wcslen(ws) : 0;
		if (len <= 0) return sf;
		sf.resize(len * 3 + 9);
		len = WideCharToMultiByte(codepage, 0, ws, (int)len, &sf[0], (int)sf.length(), 0, &used_default);
		sf.resize(len > 0 ? len : 0);
		if (has_err) *has_err = used_default ? 1 : 0;
		return sf;
	}

	struct HandleDeleter {
		void operator()(HANDLE h) const {
			if (h && h != INVALID_HANDLE_VALUE) {
				CloseHandle(h);
			}
		}
	};
	using unique_handle = std::unique_ptr<void, HandleDeleter>;
}

int CPseudoConsoleSession::create_named_pipe(int mode, HANDLE& hServer, HANDLE& hClient)
{
	constexpr DWORD kNamedPipeBufferSize = 4096*2;
	static uint32_t g_pipeSequence = 0;
	char u_name[100];
	_snprintf(u_name, sizeof(u_name), "\\\\.\\pipe\\xpty-%d-%llu-%d", GetCurrentProcessId(), GetTickCount64(), InterlockedIncrement(&g_pipeSequence));
	DWORD server_mode, cli_mode;
	switch (mode & (GENERIC_WRITE | GENERIC_READ)) {
	case GENERIC_WRITE | GENERIC_READ:
		server_mode = PIPE_ACCESS_DUPLEX;
		cli_mode = GENERIC_WRITE | GENERIC_READ;
		break;
	case GENERIC_WRITE:
		server_mode = PIPE_ACCESS_OUTBOUND;
		cli_mode = GENERIC_READ;
		break;
	case GENERIC_READ:
		server_mode = PIPE_ACCESS_INBOUND;
		cli_mode = GENERIC_WRITE;
		break;
	default:
		return -1;
	}

	HANDLE hs = CreateNamedPipeA(u_name, server_mode | FILE_FLAG_OVERLAPPED,
		PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
		1, kNamedPipeBufferSize, kNamedPipeBufferSize, 0, nullptr);
	HANDLE hc = nullptr;

	do {
		OVERLAPPED over{};
		if (!hs) break;
		BOOL bc = ConnectNamedPipe(hs, &over);
		if (!bc) {
			DWORD err = GetLastError();
			if (err != ERROR_IO_PENDING && err != ERROR_PIPE_CONNECTED) break;
		}
		hc = CreateFileA(u_name, cli_mode, 0, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
		if (!hc) {
			CancelIoEx(hs, &over);
			break;
		}
		DWORD bytes = 0;
		if (!GetOverlappedResult(hs, &over, &bytes, FALSE)) {
			Sleep(0);
			if (!GetOverlappedResult(hs, &over, &bytes, FALSE)) {
				CancelIoEx(hs, &over);
				break;
			}
		}
		Sleep(0);
		hServer = hs;
		hClient = hc;
		return 0;
	} while (0);

	if (hs) CloseHandle(hs);
	if (hc) CloseHandle(hc);
	return -1;
}


int CPseudoConsoleSession::Start(const Config& cfg, PROCESS_INFORMATION & pinfo) {
	assert(!m_hPcon && !m_hInputWrite && !m_hOutputRead);
	COORD size{ cfg.width, cfg.height };
	HANDLE output = GetStdHandle(STD_OUTPUT_HANDLE);
	memset(&pinfo, 0, sizeof(pinfo));
	if (!cfg.cmdline && !cfg.exe)
		return -1;

	if (!size.X || !size.Y) {
		size.X = 120;
		size.Y = 40;
		CONSOLE_SCREEN_BUFFER_INFO info{};
		if (!GetConsoleScreenBufferInfo(output, &info))
		{
			size.X = static_cast<SHORT>(info.srWindow.Right - info.srWindow.Left + 1);
			size.Y = static_cast<SHORT>(info.srWindow.Bottom - info.srWindow.Top + 1);
		}
	}

	BOOL br;
	HPCON rawPseudoConsole = nullptr;
	HANDLE hInputWriter = 0, hOutputReader = 0;
	{
		HANDLE hRemoteInput = 0, hRemoteOutput = 0;
		int i1 = create_named_pipe(GENERIC_WRITE, hInputWriter, hRemoteInput);
		int i2 = create_named_pipe(GENERIC_READ, hOutputReader, hRemoteOutput);
		if (i1 != 0 || i2 != 0) {
			if (hInputWriter) CloseHandle(hInputWriter);
			if (hRemoteInput) CloseHandle(hRemoteInput);
			if (hOutputReader) CloseHandle(hOutputReader);
			if (hRemoteOutput) CloseHandle(hRemoteOutput);
			return -1;
		}

		// TODO: hook this.
		HRESULT hr = ConptyCreatePseudoConsole(size, hRemoteInput, hRemoteOutput, 0, &rawPseudoConsole);
		CloseHandle(hRemoteInput);
		CloseHandle(hRemoteOutput);
		if (FAILED(hr)) {
			CloseHandle(rawPseudoConsole);
			CloseHandle(hInputWriter);
			CloseHandle(hOutputReader);
			return hr;
		}
	}

	// next, prepare attribute
	PPROC_THREAD_ATTRIBUTE_LIST ptattr = 0;
	{
		SIZE_T attributeBytes = 0;
		InitializeProcThreadAttributeList(nullptr, 1, 0, &attributeBytes);
		ptattr = (PPROC_THREAD_ATTRIBUTE_LIST)calloc(attributeBytes, 1);
		br = InitializeProcThreadAttributeList(ptattr, 1, 0, &attributeBytes);
		br = br && UpdateProcThreadAttribute(ptattr, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, rawPseudoConsole, sizeof(rawPseudoConsole), nullptr, nullptr);
		if (!br) {
			DeleteProcThreadAttributeList(ptattr);
			free(ptattr);
			ptattr = 0;
		}
	}

	if (ptattr) {
		STARTUPINFOEX sinfo{};
		sinfo.StartupInfo.cb = sizeof(sinfo);
		sinfo.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
		sinfo.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
		sinfo.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
		sinfo.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
		sinfo.lpAttributeList = ptattr;

		std::wstring wcmdline = a2w(cfg.cmdline ? cfg.cmdline : cfg.exe);
		std::wstring exe;
		LPCWSTR x_exe = nullptr;
		std::wstring workdir;
		LPCWSTR x_dir = nullptr;
		if (cfg.exe) {
			exe = a2w(cfg.exe);
			x_exe = exe.c_str();
		}
		if (cfg.workDir) {
			workdir = a2w(cfg.workDir);
			x_dir = workdir.c_str();
		}
		br = CreateProcessW(x_exe, &wcmdline[0], nullptr, nullptr, FALSE, EXTENDED_STARTUPINFO_PRESENT|CREATE_SUSPENDED,
			nullptr, x_dir, &sinfo.StartupInfo, &pinfo);
	}

	DeleteProcThreadAttributeList(ptattr);
	free(ptattr);
		// final free.
	if (!br)
	{
		CloseHandle(hInputWriter);
		CloseHandle(hOutputReader);
		ConptyClosePseudoConsole(rawPseudoConsole);
		return -1;
	}
	this->m_hPcon = rawPseudoConsole;
	this->m_hInputWrite = hInputWriter;
	this->m_hOutputRead = hOutputReader;
	if (!cfg.suspend) {
		ResumeThread(pinfo.hThread);
	}
	return 0;
}

int CPseudoConsoleSession::Resize(int width, int height) {
	if (!m_hPcon) {
		return -1;
	}
	COORD size{ (SHORT)width, (SHORT)height };
	return SUCCEEDED(ConptyResizePseudoConsole(m_hPcon, size)) ? 0 : -1;
}

void CPseudoConsoleSession::Clear() {
	if (m_hInputWrite)
		CloseHandle(m_hInputWrite);
	if (m_hOutputRead)
		CloseHandle(m_hOutputRead);
	if (m_hPcon)
		ConptyClosePseudoConsole(m_hPcon);
	this->m_hPcon = 0;
	this->m_hInputWrite = 0;
	this->m_hOutputRead = 0;
}


bool CPseudoConsoleSession::init_console() {
	SetConsoleCP(CP_UTF8);
	SetConsoleOutputCP(CP_UTF8);
	HANDLE ho = GetStdHandle(STD_OUTPUT_HANDLE);
	DWORD mode = 0;
	BOOL b1 = GetConsoleMode(ho, &mode);
	if (b1 && (mode & ENABLE_VIRTUAL_TERMINAL_PROCESSING) == 0) {
		mode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
		SetConsoleMode(ho, mode);
	}
	HANDLE hi = GetStdHandle(STD_INPUT_HANDLE);
	BOOL b2 = GetConsoleMode(hi, &mode);
	if (b2 && (mode & ENABLE_VIRTUAL_TERMINAL_INPUT) == 0) {
		mode |= ENABLE_VIRTUAL_TERMINAL_INPUT;
		SetConsoleMode(hi, mode);
	}
	atexit([]() {
		// NOTE: no need to revert stdout
		DWORD mode = 0;
		HANDLE hi = GetStdHandle(STD_INPUT_HANDLE);
		if (GetConsoleMode(hi, &mode)) {
			mode &= ~ENABLE_VIRTUAL_TERMINAL_INPUT;
			SetConsoleMode(hi, mode);
		}
	});
	return b1 || b2;
}
#endif
