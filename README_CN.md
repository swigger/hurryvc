# 背景与需求

在使用 ai时，很想能随时随地写代码。

也有别的工具解决此需求，比如 happy,hapi，但它们问题较多，hapi本来是来取代happy的，但本身也是bug一堆。它甚至不支持在弹出的选项中选一个。

另一种解决是终端复用工具。比如 tmux/wezterm等。它们的问题是配置较重，如果我大部分时间在电脑边，少量时间在外面，那么在手机上要用这些工具，虽然不是不行，但是很不方便，需要 tailscale 组网，然后再用 termius 之类的工具进行ssh连接再使用。而且，如果在电脑端使用时忘了打开term复用，那ssh连上去后也用不了原来的那个 活动的session，虽然可以强杀再resume session，但要是它还在工作呢？杀了不可惜吗。

所以，最终决定做一个简单的终端服务工具，提供生产端跨平台(win/linux/macos)且轻量级配置，移动端只需要网页0配置。以本地工作为主。典型网络结构比如：公司电脑A--路上--家里电脑B。工作主进程在A上运作，路上可以用手机短暂控制。回到家里用B电脑远程连接A电脑，继续在A上操作。如果操作比较简单，也可以直接在B电脑上用网页短暂控制。

定义电脑A为生产端，手机/电脑B为消费端。

因此，本程序的设计目标是用在消费端打一定折扣的体验，换取消费端的0配置开箱即用。它的目的不是取代 tmux/wezterm/screen等终端复用工具。

本程序取为叫 `hurryvc` ，是Hurry Vibe Coding 的意思。



# hurryvc的运行模式

1. server模式，它接受生产端推送term变更数据，同步给消费端，接受消费端的输入推送，合并后提交给生产端作为用户输入。如果同时有多个消费端输入数据，可能导致数据混乱，这不是bug，是设计如此。

2. 生产模式，此时我们称这个hurryvc为生产者。运行 hurryvc codex/cluade/sh/pwsh 等命令。支持windows/mac/linux。

3. 消费模式，严格来说，这是server模式的一部分功能。因为此时hurryvc提供http接口由浏览器运行js共同完成消费侧功能。用户可查看当前的session list。选择一个session后，可以在页面实时看到term内容变更。也可以输入文字或控制键。




# 安全设计

server运行时需要（首次自动生成）master key，用户可以修改 `~/.config/hurryvc/server.json5` 指定简单的密码作为master key。

生产者和消费端必须提供master key才能与server链接。

生产者向server注册时，还要提供production-group key。生产者首次运行自动生成，也可以由用户更改 `~/.config/hurryvc/run.json5` 指定简单密码。p的存在使得一个server可以向多个用户提供服务，用户A就看不到用户B的终端。

消费端即网页端有清理key的按钮，暂时借用朋友手机操作后可以清除key，最终不留下记录。

# 技术设计

使用 rust 为主体语言，实现大部分逻辑。

使用c++作为辅助语言，实现一些跟平台相关的内容，使用directcpp包装为rust接口函数，供rust使用。

使用 vue为页面端框架，提供终端数据流的带颜色和格式显示。提供消费端的选择和控制。


# 编译方法

前提：安装 nodejs/cargo，windows 编译还需要安装visual studio。

```bash
cd hurry-ui
npm install
npm run build
cd ..
cargo build --release
```

Windows注意： 需要把 Openconsole.exe 拷到 target/debug, target/release或者你放置主文件的地方。保证openconsole.exe与主程序在同一个目录。

# 运行方法

首先运行server:

`hurryvc server`

然后主力运行：

`hurryvc run -- codex`

这里codex可以是别的命令，比如claude/droid，当然也可以是 /bin/fish, pwsh等。

接下来去 ~/.config/hurryvc 查看你的 server master key和 run group_key (就是前面说的p-key)

在 http://<your-ip>:6600 就可以登录了。

如何在地铁或者外地或者远程访问呢？这就需要你使用 tailscale/cloud flare等把你的6600端口转发到公共网址了。

等等！前面不是说这个工具的目的是不使用这些吗？呃，如果你用tailscale那手机上还是得设置，但如果用cloudflare手机上就不需要初始设置了。当然，也有不用这两个的办法：

请你的朋友在服务器上运行 hurryvc server，然后把 127.0.0.1:6600 用nginx等工具转到 https://example.com/some/path

然后，你在电脑上无需运行 hurryvc server，只需要运行：

```bash
hurryvc.exe run --server https://example.com/some/path --master-key the-master-key -- /bin/fish
```
你就可以登录 https://example.com/some/path 进行远程操作了。
