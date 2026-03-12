# Background & Motivation

When using AI, I really want to be able to code anytime, anywhere.

There are other tools that address this need, such as happy and hapi, but they have many issues. hapi was meant to replace happy, but it's also full of bugs. It doesn't even support selecting from a popup option list.

Another approach is terminal multiplexer tools like tmux/wezterm. The problem is they require heavy configuration. If I spend most of my time at my computer and only occasionally work remotely, using these tools on a phone is inconvenient — you need tailscale for networking, then use something like termius for SSH. Also, if you forget to start a terminal multiplexer on the desktop, you can't access the active session after SSH-ing in. You could force-kill and resume the session, but what if it's still working? That would be a waste.

So I decided to build a simple terminal service tool: cross-platform (Windows/Linux/macOS) and lightweight on the producer side, zero-config on the consumer side (just a web page). It's designed for local-first workflows. A typical network setup: Office PC A -- on the go -- Home PC B. The main process runs on A, you can briefly control it from your phone on the go, then connect from PC B at home to continue working on A. For simple operations, you can also use a web page on PC B for quick control.

PC A is defined as the producer, and the phone/PC B as the consumer.

The design goal is to trade a slightly reduced experience on the consumer side for zero-config, out-of-the-box usability. It is not meant to replace tmux/wezterm/screen or other terminal multiplexers.

The program is called `hurryvc`, which stands for Hurry Vibe Coding.



# Operating Modes

1. Server mode: accepts terminal change data pushed from the producer, syncs it to consumers, accepts input from consumers, merges it, and forwards it to the producer as user input. If multiple consumers send input simultaneously, data may get mixed up — this is by design, not a bug.

2. Producer mode: in this mode, hurryvc acts as a producer. Run commands like `hurryvc codex/claude/sh/pwsh`, etc. Supports Windows/macOS/Linux.

3. Consumer mode: strictly speaking, this is part of server mode. hurryvc provides HTTP endpoints, and the browser runs JavaScript to complete the consumer-side functionality. Users can view the current session list, select a session, see real-time terminal content updates, and send text or control keys.



# Security Design

The server requires a master key (auto-generated on first run). Users can modify `~/.config/hurryvc/server.json5` to set a simple password as the master key.

Both the producer and consumer must provide the master key to connect to the server.

When registering with the server, the producer must also provide a production-group key. This is auto-generated on first run and can be changed in `~/.config/hurryvc/run.json5`. The production-group key allows a single server to serve multiple users — user A cannot see user B's terminals.

The consumer (web page) has a button to clear keys, so after temporarily borrowing a friend's phone, you can clear the keys and leave no trace.

# Technical Design

Rust is the primary language, implementing most of the logic.

C++ is used as a supplementary language for platform-specific functionality, wrapped into Rust interface functions via the directcpp crate.

Vue is used for the web frontend, providing colored and formatted display of terminal data streams, as well as consumer-side selection and control.


# Build Instructions

Prerequisites: Install Node.js, python and Cargo. Building on Windows also requires Visual Studio.

```bash
cd hurry-ui
npm install
npm run build
cd ..
cargo build --release
```

Windows note: You need to copy `OpenConsole.exe` to `target/debug`, `target/release`, or wherever you place the main binary. Ensure `OpenConsole.exe` is in the same directory as the main program.

# Usage

First, start the server:

`hurryvc server`

Then run your main command:

`hurryvc run -- codex`

Here `codex` can be replaced with other commands like `claude`/`droid`, or even `/bin/fish`, `pwsh`, etc.

Next, check `~/.config/hurryvc` for your server master key and run group_key (the production-group key mentioned above).

Log in at `http://<your-ip>:6600`.

How to access remotely (e.g., on the subway or from another location)? You'll need to use tailscale/cloudflare or similar to forward port 6600 to a public URL.

Wait — didn't we say the goal was to avoid all that? Well, if you use tailscale, you still need to set up the phone. But with cloudflare, no initial setup is needed on the phone. There's also another way without either:

Ask a friend to run `hurryvc server` on their server, then use nginx or similar to proxy `127.0.0.1:6600` to `https://example.com/some/path`.

Then, on your computer, you don't need to run `hurryvc server` — just run:

```bash
hurryvc.exe run --server https://example.com/some/path --master-key the-master-key -- /bin/fish
```

And you can log in at `https://example.com/some/path` for remote access.
