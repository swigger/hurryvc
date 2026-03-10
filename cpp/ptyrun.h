#pragma once
#ifdef _WIN32
extern "C" {
    HRESULT WINAPI ConptyCreatePseudoConsole(
        _In_ COORD size,
        _In_ HANDLE hInput,
        _In_ HANDLE hOutput,
        _In_ DWORD dwFlags,
        _Out_ HPCON* phPC
    );
    HRESULT WINAPI ConptyResizePseudoConsole(
        _In_ HPCON hPC,
        _In_ COORD size
    );
    void WINAPI ConptyClosePseudoConsole(HPCON hPC);
}

class CPseudoConsoleSession
{
public:
    struct Config {
        // default to current console size if 0 inited.
        int width;
        int height;

		// process creation parameters
        const char* exe;
        const char* cmdline;
        const char* workDir;
        bool suspend;
    };
    int Start(const Config& cfg, PROCESS_INFORMATION & pinfo);
    int Resize(int width, int height);
    void Clear();
    CPseudoConsoleSession() = default;
    ~CPseudoConsoleSession() { Clear(); }

public:
    HPCON m_hPcon = nullptr;
    HANDLE m_hInputWrite = nullptr;
    HANDLE m_hOutputRead = nullptr;
    static bool init_console();

protected:
    int create_named_pipe(int mode, HANDLE &hServer, HANDLE &hClient);
};
#endif
