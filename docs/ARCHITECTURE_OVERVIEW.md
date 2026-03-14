# ch1-T2L3 软件架构总览

本文描述 `tg-rcore-tutorial-ch1-T2L3` 作为一个独立软件包的实现结构、执行路径与模块分工。

## 1. 系统定位

`ch1-T2L3` 是一个运行在 RISC-V S 态的 `no_std` 裸机内核最小样例，功能包括：

- 最小启动（手动设栈）；
- SBI 输出与关机；
- VirtIO Block 单扇区写读校验。

该项目强调“教学可读性与最小闭环”，不是通用块设备栈。

---

## 2. 目录与模块职责

```text
tg-rcore-tutorial-ch1-T2L3/
├── .cargo/config.toml         # 目标平台与 QEMU runner（自动创建空白 disk.img）
├── build.rs                   # 生成 linker.ld，定义 M/S 态关键段布局
├── Cargo.toml                 # 包信息与依赖（tg-sbi）
├── README.md                  # 章节说明文档
└── src/
    ├── main.rs                # 入口、主流程、错误收口
    ├── virtio_mmio.rs         # VirtIO MMIO（legacy v1）寄存器层
    ├── virtqueue.rs           # VirtQueue 单请求同步轮询实现
    └── virtio_blk.rs          # 块设备封装（read_block/write_block）
```

---

## 3. 分层架构

```text
应用流程（main.rs）
      │
      ▼
块设备接口（virtio_blk.rs）
      │
      ▼
队列管理（virtqueue.rs）
      │
      ▼
MMIO 寄存器访问（virtio_mmio.rs）
      │
      ▼
QEMU virtio-blk-device + disk.img
```

### 3.1 `main.rs`：应用编排层

负责启动后的业务流程：
1. 初始化 `VirtIOBlk`；
2. 构造 512B 写缓冲，填入 `Hello, world!`；
3. 写入扇区 1；
4. 读回扇区 1；
5. 比较一致性并输出结果；
6. 通过 SBI 正常/异常关机。

### 3.2 `virtio_blk.rs`：设备语义层

将“块读写”抽象为方法调用：
- `new()`：完成设备探测、状态机推进、queue 初始化；
- `write_block()`：提交 OUT 请求并等待完成；
- `read_block()`：提交 IN 请求并等待完成。

此层不关心具体寄存器偏移和 ring 细节，依赖下层能力。

### 3.3 `virtqueue.rs`：数据通道层

负责 VirtQueue 内存组织和请求提交：
- 维护描述符表、avail ring、used ring；
- 构建三段描述符链（header/data/status）；
- 通过 `used.idx` 轮询完成；
- 处理 legacy 需要的页对齐约束。

### 3.4 `virtio_mmio.rs`：硬件访问层

封装 volatile 寄存器读写：
- 设备探测（magic/version/device id）；
- 功能位协商（当前最小实现选择 0）；
- 状态寄存器推进；
- legacy queue 配置：
  - `guest_page_size`
  - `queue_num`
  - `queue_align`
  - `queue_pfn`
- queue notify 与 interrupt ack。

---

## 4. 启动与运行流程

## 4.1 构建期（build-time）

- `build.rs` 在 RISC-V 目标下生成 linker 脚本；
- 链接脚本安排：
  - M-mode 区域（由 `tg-sbi` 使用）
  - S-mode 区域（本程序 `_start` 与后续代码）

## 4.2 运行期（run-time）

1. `_start` 设栈并跳转 `rust_main`；
2. `rust_main` 调用 `VirtIOBlk::new()`；
3. `VirtIOBlk` 通过 MMIO + VirtQueue 完成请求提交；
4. 成功后打印 `verify ok` 并 `shutdown(false)`。

---

## 5. 配置与外部依赖

### 5.1 关键配置

`.cargo/config.toml`：
- 默认目标：`riscv64gc-unknown-none-elf`；
- runner 使用 `bash -lc`：
  - 先 `truncate` 生成空白 `disk.img`；
  - 再启动 `qemu-system-riscv64` 并挂载 `virtio-blk-device`。

### 5.2 外部依赖

- `tg-sbi`：提供 `console_putchar` / `shutdown`；
- QEMU `virt` 平台：提供 MMIO VirtIO Block 设备。

---

## 6. 当前实现边界

为了保持教学最小实现，当前未实现：
- 中断驱动（仅轮询）；
- 并发多请求与队列复用；
- 缓存层、分区层、文件系统对接；
- modern MMIO v2 路径。

这使代码更短、更容易追踪，但吞吐与通用性有限。

---

## 7. 可扩展方向

后续若要继续演进，可按以下顺序：

1. 在 `virtqueue.rs` 支持多请求 inflight；
2. 在 `virtio_blk.rs` 加中断完成路径（减少轮询占用）；
3. 抽象 `BlockDevice trait` 以便挂接 easy-fs；
4. 增加 modern v2 初始化分支，提高兼容性。
