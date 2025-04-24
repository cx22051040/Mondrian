## Smithay - 实现简易的 Compositor

### 简介

> Smithay aims to provide building blocks to create wayland compositors in Rust. While not being a full-blown compositor, it'll provide objects and interfaces implementing common functionalities that pretty much any compositor will need, in a generic fashion.

`Smithay` 使用 `rust` 封装了使用 `Wayland` 协议的一些基本内容，实现了核心 `Wayland` 协议以及一些重要的协议。

[https://github.com/Smithay/smitha](https://github.com/Smithay/smithay)

本文的基础框架代码：[TODO](TOOD)

### EventLoop 事件分发机制

在基于 `Smithay` 构建的 `Wayland Compositor` 中，`事件循环（EventLoop）`是整个系统运行的核心。所有的输入、输出、客户端请求、时间驱动逻辑，乃至后台任务的调度都依赖于该机制完成事件的监听与响应。

### 架构概览

Smithay 采用 `calloop` 作为主事件循环框架，其优势在于：

- 可插拔式事件源管理（source registration）
- 高性能的非阻塞式事件分发
- 原生支持定时器、通道等常用异步通信模型

`Smithay` 为 `Winit` 后端提供了优秀的兼容模式，可以很方便的进行开发。

##### 定义

在 `main` 函数中定义一个 `EventLoop` 主体非常简单，直接调用相关的库函数：

```rust
use smithay::reexports::calloop::EventLoop;
let mut event_loop: EventLoop<'_, State> = EventLoop::try_new().unwrap();
```

在这里，`State` 类型是全局状态结构体，由我们自己定义，目前暂时不谈论细节，你只需知道这个结构体管理所有的程序状态即可。

##### 事件源插入

通过获取 `LoopHandle` 就来执行事件的插入，删除与执行操作：

```rust
event_loop
    .handle() // LoopHandle
    .insert_source(input_backend, move |event, &mut metadata, state| {
        // action
    })?;
```

在这里，我们通过 `handle()` 函数获取操作入口，使用 `insert_source` 函数来注册 `事件源（EventSource）`，其会将一个监听对象添加到主循环中，并且绑定一个处理函数（回调闭包），每当事件产生时，就会调用这个函数。

事件循环可以绑定多个事件源，常见类别如下：

| 类型 | 来源 | 示例事件 |
|------|------|----------|
| 输入设备 | libinput | PointerMotion、KeyboardKey 等 |
| 图形输出 | DRM/KMS, Winit | 热插拔、显示尺寸改变 |
| Wayland 客户端 | WaylandSocket | 请求窗口创建、buffer attach |
| 定时器 | calloop Timer | 动画帧调度、超时 |
| 自定义通道 | calloop Channel | 后台任务返回、信号触发 |

在 `insert_source` 中绑定的回调闭包具有以下签名：
```rust
FnMut(E, &mut Metadata, &mut State)
```
- `E`: 来自事件源的事件本体，类型依赖于事件源。
- `Metadata`: 事件元信息（通常是 `calloop::generic::GenericMetadata`），包含事件触发时的底层 I/O 状态，例如可读/可写标志。大多数情况下你可以忽略该参数，除非你要做更底层的 I/O 操作。
- `State`: 传入的全局状态对象，是你自定义的全局状态结构，也就是一开始定义的类型 `EventLoop<'_, State>` 中的 `State`。

*或许你会疑惑我们只是告诉了 `EventLoop` 的 `State` 类型，没有实现 `State` 值的传入，为什么这里可以获取到一个可变借用，别着急，后面就会揭晓答案*

最容易理解的就是客户端连接请求的事件处理：

```rust
let source = ListeningSocketSource::new_auto().unwrap();
let socket_name = source.socket_name().to_string_lossy().into_owned();
loop_handle
    .insert_source(source, move |client_stream, _, state| {
        state
            .display_handle
            .insert_client(client_stream, Arc::new(ClientState::default()))
            .unwrap();
    })
    .expect("Failed to init wayland socket source.");
```

`Wayland` 是一个基于 `UNIX 域套接字（UNIX domain socket）` 的通信协议，`Client` 与 `Compositor` 之间的所有协议交互，都是通过一个共享的本地套接字进行的。

`ListeningSocketSource::new_auto()` 会自动创建一个新的 `UNIX 域套接字`，并监听客户端连接请求。默认在 `/run/user/UID/` 下创建 `socket` 文件，例如 `wayland-0`.本地调试时我们需要设置环境变量 `WAYLAND_DISPLAY=wayland-0` 来绑定测试的 `Compositor`。

当有客户端连接或请求发生时，对应的事件将触发该回调闭包，并调用 `.display_handle.insert_client` 以执行客户端初始化、资源绑定或协议处理等逻辑。

详细的创建内容在 [Client事件源]() 篇会详细讲解。

##### 事件执行

此前我们只是将需要监听的事件源和需要执行的函数内容加入到了 `EventLoop` 中，但还未真正的下达指令 - 你可以开始监听了，因此，我们还需要以下代码来真正开启循环：

```rust
event_loop
    .run(None, &mut state, move |_| {
        //  is running
    })
    .unwrap();
```

此时，我们可以解答在事件源插入中遗留的问题了，可变借用是此时才被传入其中的，顺序上也许会让人疑惑，但这就是 Rust 的“延迟状态绑定”机制的奇妙之处。

在调用 `insert_source` 时，事件循环尚未开始运行，只是注册了事件源与回调；

所有回调的 `state` 参数类型由 `EventLoop<T>` 的泛型 T 决定（例如我们定义的 `State`），但值本身尚未存在；

直到调用 `run(&mut state, ...)` 这一刻，`state` 的实际引用才被注入到事件循环中；

从此刻开始，`calloop` 内部在每次事件分发时，才会将这个 `&mut T` 传入闭包中。

它确保了事件循环中所有 `state` 的使用都在 `run()` 的生命周期范围内发生，且绝不会出现悬垂引用或数据竞争。

至此，核心的框架就已经被我们解决了，接下来就是真正的进行对不同事件源的处理。

### Client 事件源

在 `Wayland` 协议中，客户端的渲染行为是以 `wl_surface` 为基本单位的。每一个客户端窗口本质上都是在创建并提交一个或多个 `surface`，而这些 `surface` 的行为则由其绑定的角色（如 `xdg_toplevel` 或 `xdg_popup` ）所定义。

在之前我们已经见过以下的代码：

```rust
let source = ListeningSocketSource::new_auto().unwrap();
let socket_name = source.socket_name().to_string_lossy().into_owned();
loop_handle
    .insert_source(source, move |client_stream, _, state| {
        state
            .display_handle
            .insert_client(client_stream, Arc::new(ClientState::default()))
            .unwrap();
    })
    .expect("Failed to init wayland socket source.");
```

这段代码创建了一个新的 `UNIX 域套接字`，监听客户端的连接请求。`Wayland` 是一个 `拉模型（pull model）`，客户端不会主动创建窗口，而是从服务端请求对象并获得引用。其中具体的请求过程如下：

- 连接 `display socket`：客户端连接 `compositor` 暴露的 `UNIX 域套接字`（如 `/run/user/1000/wayland-0`）。
- 绑定 `wl_registry`：连接后，客户端首先获取 `wl_display` 提供的 `wl_registry` `对象，它包含了compositor` 所支持的所有全局对象（如 `wl_compositor`、`wl_shm`、`xdg_wm_base` 等）。
- 获取 `wl_compositor` 接口：客户端通过 `wl_registry.bind(...)` 获得 `wl_compositor` 接口，允许创建 `wl_surface`。
- 创建 `wl_surface`：客户端通过 `wl_compositor.create_surface()` 调用，获得一个新的 `surface` 例，这是所有图形内容提交的基础。
- 绑定 `xdg_surface` 与角色：随后，客户端使用 `xdg_wm_base.get_xdg_surface(surface)` 创建`xdg_surface`，再通过 `get_toplevel()` 等调用为其赋予具体角色。
- 随后就可以通过 `surface.commit()` 向 `compositor` 传递需要显示的内容。

我们可以用一个简单的图解来直观的展示建立连接的过程：

TODO

看到如此多的协议信息，不要害怕！让我们一步步的来拆解各个步骤的具体含义，首先有必要介绍一下 `xdg-shell` 协议。

#### xdg-shell 协议实现

- [https://wayland.app/protocols/xdg-shell](https://wayland.app/protocols/xdg-shell)

在 `Wayland` 协议体系中，`xdg-shell` 是一项核心协议，扩展了基础的 `wl_surface` 对象，使其能够在桌面环境下扮演窗口的角色。它是现代 `Wayland` 桌面应用窗口管理的标准，涵盖了顶层窗口、弹出窗口、窗口状态控制等一系列行为。

`xdg-shell` 协议主要围绕以下对象展开：
- `xdg_wm_base`：客户端首先通过 `wl_registry` 获取 `xdg_wm_base` 接口。
- `xdg_surface`：通过 `xdg_wm_base.get_xdg_surface(wl_surface)`，客户端将一个基础的 `wl_surface` 与 `xdg_surface` 关联起来。
- `xdg_toplevel`：通过 `xdg_surface.get_toplevel()`，该 `surface` 被赋予了「顶层窗口」的角色。
- `xdg_popup`：替代 `toplevel`，它赋予窗口「弹出窗口」的角色，通常用于菜单、右键栏等临时 UI。

一个 `wl_surface` 只能被赋予一个角色，即它要么是 `xdg_toplevel`，要么是 `xdg_popup`，不能同时拥有或重复绑定。

我们可以这样理解：`wl_surface` 是原始画布，`xdg_surface` 是语义包装器，`xdg_toplevel` 或 `xdg_popup` 是具体的行为描述者。

##### configure / ack 机制

在 `xdg-shell` 协议中，一个非常重要的机制就是「双向确认机制」：

在有修改需求的时候，`compositor` 发起 `configure` 事件，告知客户端窗口大小、状态变更请求，客户端必须回应 `ack_configure`，明确表示接收到该配置并将进行重绘，只有在 `ack` 后，客户端提交的 `surface.commit()` 内容才会被正式展示。

这种机制是 `Wayland` 相对于传统 `X11` 的一大改进点，确保了服务端与客户端状态始终一致，**不会出现窗口闪动或布局错乱**。

```rust
use smithay::{
    delegate_xdg_shell,
    wayland::shell::xdg::{XdgShellHandler, XdgShellState},
};

// init in state struct
{
    let xdg_shell_state = XdgShellState::new::<Self>(&display_handle);
}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        //
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        //
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        //
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        //
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        //
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        //
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        //
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) { }
}
delegate_xdg_shell!(State);
```

设置 `xdg-shell` 协议的相关代码也非常简单，只需要使用 `smithay` 提供的框架即可。具体函数内部实现的方法，参考基础框架代码。

至此，我们已经完成了核心的 `surface` 分配机制，相当于给画家提供了画板，还设置了画板最后展出的场馆 - `toplevel` 或 `popup` 。

### input 事件源

`compositor` 的核心职责之一是处理来自用户的输入事件，如鼠标移动、按键、触摸交互等。而这些输入事件的来源方式依赖于 `compositor` 所使用的后端类型。`Smithay` 提供了多个后端支持，其中包括：

- `winit` 后端：通常用于开发阶段，快速接入图形窗口系统并获取输入；
- `TTY` + `libinput` 后端：更贴近生产环境，直接从内核设备文件读取输入事件，适用于 DRM/KMS 渲染路径。

#### 使用 winit 后端的 input 事件源

在 `winit` 模式下，`Smithay` 提供了高度集成的 `WinitInputBackend` 类型，开发者可以非常方便地将其作为事件源插入 `EventLoop` 中，例如：

```rust
event_loop
    .handle()
    .insert_source(winit_backend, move |event, _, state| {
        state.process_input_event(event);
    })?;
```

`winit` 后端封装了窗口事件与输入事件，并提供统一的接口输出 `InputEvent`。`Smithay` 内部支持对这些事件进行标准化转换，如：

- `PointerEvent`
- `KeyboardEvent`
- `TouchEvent`

通常在 `state.process_input_event` 函数中进行分发，`Smithay` 的 `seat` 抽象会帮助我们自动处理焦点跟踪、输入分发、键盘修饰等细节。

```rust
use smithay::backend::{delegate_seat, input::{Seat, SeatHandler, SeatState}};

// init in state struct
{
    let mut seat_state = SeatState::new();
    let seat_name = String::from("winit");
    let mut seat: Seat<Self> = seat_state.new_wl_seat(display_handle,seat_name);
}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        //
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        image: smithay::input::pointer::CursorImageStatus,
    ) {
        //
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        //
    }
}
delegate_seat(State);
```

#### 使用 TTY 后端的 input 事件源

在没有图形服务器支持的**裸机环境**下，我们通常使用 `TTY` 作为图形输出后端，并结合 `libinput` 获取来自 `/dev/input` 的事件。此时输入处理方式较为底层，需要我们显式构造事件源：

``` rust
let libinput_context = Libinput::new_with_udev(...);
let input_backend = LibinputInputBackend::new(libinput_context, seat, ...);
```

与 `winit` 不同，`libinput` 后端需要手动处理权限和 `seat` 初始化，但优点在于：

- 支持更精细的输入设备管理；
- 能兼容热插拔、多用户、多 `seat`；
- 更贴近真实硬件行为（如 VT 切换、KMS 挂载）；

事件注册仍然可以通过 `insert_source` 完成。

```rust
event_loop
    .handle()
    .insert_source(input_backend, move |event, _, state| {
        state.process_input_event(event);
    })?;
```

无论是 `winit` 还是 `TTY` 模式，输入事件的处理流程基本保持一致：

- 后端产生 `InputEvent`；
- 事件被传入 `compositor` 的状态处理器；
- `Smithay` 的 `Seat` 接口会自动更新焦点状态、生成 `Wayland` 协议事件；
- 若存在活跃客户端，事件会通过 `wl_pointer`、`wl_keyboard`、`wl_touch` 等接口传输至客户端。

具体的状态如： `InputEvent::Keyboard`，`InputEvent::PointerMotion` 等这里不再详细讲解，具体参考基础框架代码内容。

至此，我们就得到了一个简单的，可以响应客户端请求，并且支持鼠标，键盘操作的简易 Wayland Compositor。

### 参考资料

- [https://github.com/Smithay/smithay](https://github.com/Smithay/smithay)
- [https://crates.io/crates/smithay](https://crates.io/crates/smithay)
- [https://docs.rs/smithay/latest/smithay/](https://docs.rs/smithay/latest/smithay/)