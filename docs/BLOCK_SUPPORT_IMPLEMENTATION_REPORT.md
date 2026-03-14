# ch1-T2L3 Block 支持实现报告

## 1. 目标与约束

本次需求：在 `ch1-T2L3` 中增加最小块设备能力，不依赖 `virtio-drivers`，仅完成以下闭环：

1. 初始化一个 VirtIO Block 设备；
2. 写入一个扇区（512B），内容包含 `Hello, world!`；
3. 读回同一扇区并字节级比较；
4. 成功正常关机，失败异常关机。

约束：
- 保持 `ch1` 风格（`no_std` / `no_main`，最小功能）；
- 不引入文件系统、不引入中断驱动；
- 单请求、同步轮询即可。

---

## 2. 规划（Plan）

分四层最小实现：

1. **MMIO 寄存器层**（`virtio_mmio.rs`）
   - 负责寄存器读写、状态位推进、queue 配置、通知与中断 ACK。
2. **VirtQueue 层**（`virtqueue.rs`）
   - 维护描述符表、avail/used ring，提交请求并轮询完成。
3. **块设备层**（`virtio_blk.rs`）
   - 对外暴露 `new/read_block/write_block`。
4. **应用入口层**（`main.rs`）
   - 调用块设备写/读/比对，打印日志并关机。

运行层配套：
- 在 `.cargo/config.toml` 的 runner 中挂载 `virtio-blk-device`；
- 每次 `cargo run` 前自动 `truncate` 空白 `disk.img`。

---

## 3. 实现（Implementation）

### 3.1 新增模块

- `src/virtio_mmio.rs`
  - 实现 VirtIO MMIO 关键寄存器读写；
  - 采用 **legacy v1** 初始化流程：
    - 设备探测（magic/version/device id）；
    - 状态推进（ACKNOWLEDGE -> DRIVER -> FEATURES_OK -> DRIVER_OK）；
    - 队列配置（`guest_page_size`, `queue_num`, `queue_align`, `queue_pfn`）。

- `src/virtqueue.rs`
  - 固定 `QUEUE_SIZE=8`；
  - 三段描述符链：`header -> data -> status`；
  - 轮询 `used.idx` 判定请求完成；
  - 按 legacy 规范处理 ring 布局与对齐。

- `src/virtio_blk.rs`
  - `VirtIOBlk::new()` 初始化设备；
  - `write_block(sector, &[u8;512])`；
  - `read_block(sector, &mut [u8;512])`；
  - 返回自定义错误 `BlkError::{Mmio, Queue, Io}`。

### 3.2 入口接入

- `src/main.rs`
  - 初始化驱动；
  - 将 `Hello, world!` 放入 `write_buf`，写入 `TEST_SECTOR=1`；
  - 读回到 `read_buf` 后比较；
  - 成功输出 `verify ok` 并 `shutdown(false)`。

### 3.3 运行配置

- `.cargo/config.toml`
  - runner 增加：
    - `truncate -s 8M .../disk.img`（每次运行自动空盘）；
    - `-drive ...` + `-device virtio-blk-device...`（挂载块设备）。

---

## 4. 测试与验证（Test）

### 4.1 编译验证

- 执行 `cargo check`，修复 `deny(warnings)` 下报错，确保本 crate 可编译。

### 4.2 运行验证

- 执行 `cargo run`，验证串口日志顺序：
  1. `[ch1] init virtio-blk...`
  2. `[ch1] write sector...`
  3. `[ch1] read sector...`
  4. `[ch1] verify ok: sector data matched`

观察到最终成功日志，说明写入与读取一致。

---

## 5. 调试过程（Debug）

### 问题 1：依赖版本冲突导致无法构建

现象：
- `failed to select a version for tg-rcore-tutorial-sbi = "^0.4.8"`

原因：
- 本地 `tg-rcore-tutorial-sbi` 版本是 `0.4.5`，而 `ch1/ch1-T2L3` 依赖声明是 `0.4.8`。

修复：
- 将 `tg-rcore-tutorial-sbi/Cargo.toml` 版本对齐到 `0.4.8`。

### 问题 2：MMIO 版本判断过严

现象：
- 运行时报错：`virtio mmio version is not 2`。

原因：
- QEMU 返回的是 legacy MMIO `version=1`。

修复：
- 驱动改为使用 legacy v1 初始化路径（最终简化为仅支持 v1）。

### 问题 3：写请求阶段卡住

现象：
- 打印到 `write sector...` 后不再前进。

原因：
- legacy virtqueue 要求 `used ring` 页对齐，初始内存布局不满足规范。

修复：
- 在 `virtqueue.rs` 中为 `used ring` 增加页对齐 padding，满足 legacy 布局要求。

### 问题 4：重复运行需要手动准备磁盘

现象：
- 用户希望每次运行都是空盘，不想手动创建镜像。

修复：
- 在 runner 中加入 `truncate`，实现每次 `cargo run` 自动生成空白磁盘文件。

---

## 6. 最终结果

已满足需求：
- 无 `virtio-drivers` 依赖；
- ch1 最小实现风格；
- 单扇区写入 `Hello, world!` 并读回校验成功；
- `cargo run` 自动生成空白磁盘并启动验证。

当前实现是“教学最小可用版本”，便于继续演进到中断驱动、多请求队列、文件系统接入等后续目标。
