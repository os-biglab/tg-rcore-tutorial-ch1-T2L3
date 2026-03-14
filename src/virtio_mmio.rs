//! VirtIO MMIO 寄存器访问层（最小实现）。
//!
//! 仅覆盖 ch1 读写一个块设备扇区所需的寄存器与状态流程。

use core::ptr::{read_volatile, write_volatile};

/// VirtIO MMIO 基地址（QEMU virt 平台设备 0）。
pub(crate) const VIRTIO0: usize = 0x1000_1000;

const MAGIC_VALUE: u32 = 0x7472_6976;
const VERSION_1: u32 = 1;
const DEVICE_ID_BLOCK: u32 = 2;

const MMIO_MAGIC_VALUE: usize = 0x000;
const MMIO_VERSION: usize = 0x004;
const MMIO_DEVICE_ID: usize = 0x008;
const MMIO_VENDOR_ID: usize = 0x00c;
const MMIO_DEVICE_FEATURES: usize = 0x010;
const MMIO_DEVICE_FEATURES_SEL: usize = 0x014;
const MMIO_DRIVER_FEATURES: usize = 0x020;
const MMIO_DRIVER_FEATURES_SEL: usize = 0x024;
const MMIO_GUEST_PAGE_SIZE: usize = 0x028;
const MMIO_QUEUE_SEL: usize = 0x030;
const MMIO_QUEUE_NUM_MAX: usize = 0x034;
const MMIO_QUEUE_NUM: usize = 0x038;
const MMIO_QUEUE_ALIGN: usize = 0x03c;
const MMIO_QUEUE_PFN: usize = 0x040;
const MMIO_QUEUE_NOTIFY: usize = 0x050;
const MMIO_INTERRUPT_STATUS: usize = 0x060;
const MMIO_INTERRUPT_ACK: usize = 0x064;
const MMIO_STATUS: usize = 0x070;

/// Status bit: 设备已被驱动识别。
pub(crate) const STATUS_ACKNOWLEDGE: u32 = 1;
/// Status bit: 设备已被驱动接管。
pub(crate) const STATUS_DRIVER: u32 = 2;
/// Status bit: 驱动功能协商完成。
pub(crate) const STATUS_FEATURES_OK: u32 = 8;
/// Status bit: 驱动初始化完成，可开始工作。
pub(crate) const STATUS_DRIVER_OK: u32 = 4;
/// Status bit: 设备发生故障。
pub(crate) const STATUS_FAILED: u32 = 128;

/// VirtIO MMIO 访问封装。
pub(crate) struct VirtioMmio {
    base: usize,
}

impl VirtioMmio {
    /// 创建 MMIO 寄存器访问器。
    pub(crate) const fn new(base: usize) -> Self {
        Self { base }
    }

    /// 校验 QEMU VirtIO 块设备存在性。
    pub(crate) fn probe_block_device(&self) -> Result<(), &'static str> {
        if self.read32(MMIO_MAGIC_VALUE) != MAGIC_VALUE {
            return Err("virtio magic mismatch");
        }
        let version = self.read32(MMIO_VERSION);
        if version != VERSION_1 {
            return Err("virtio mmio version is not legacy(v1)");
        }
        if self.read32(MMIO_DEVICE_ID) != DEVICE_ID_BLOCK {
            return Err("virtio device is not block");
        }
        let _ = self.read32(MMIO_VENDOR_ID);
        Ok(())
    }

    /// 读取设备功能位（32 位片段）。
    pub(crate) fn read_device_features(&self, sel: u32) -> u32 {
        self.write32(MMIO_DEVICE_FEATURES_SEL, sel);
        self.read32(MMIO_DEVICE_FEATURES)
    }

    /// 写入驱动功能位（32 位片段）。
    pub(crate) fn write_driver_features(&self, sel: u32, value: u32) {
        self.write32(MMIO_DRIVER_FEATURES_SEL, sel);
        self.write32(MMIO_DRIVER_FEATURES, value);
    }

    /// 读取状态寄存器。
    pub(crate) fn status(&self) -> u32 {
        self.read32(MMIO_STATUS)
    }

    /// 写入状态寄存器。
    pub(crate) fn set_status(&self, value: u32) {
        self.write32(MMIO_STATUS, value);
    }

    /// 增量设置状态位。
    pub(crate) fn add_status(&self, bits: u32) {
        self.set_status(self.status() | bits);
    }

    /// 选择当前操作的 VirtQueue。
    pub(crate) fn set_queue_sel(&self, queue: u32) {
        self.write32(MMIO_QUEUE_SEL, queue);
    }

    /// 当前选择队列支持的最大长度。
    pub(crate) fn queue_num_max(&self) -> u32 {
        self.read32(MMIO_QUEUE_NUM_MAX)
    }

    /// 设置当前选择队列长度。
    pub(crate) fn set_queue_num(&self, queue_size: u32) {
        self.write32(MMIO_QUEUE_NUM, queue_size);
    }

    /// 设置 Legacy 接口所需的 guest 页大小（通常 4096）。
    pub(crate) fn set_guest_page_size(&self, size: u32) {
        self.write32(MMIO_GUEST_PAGE_SIZE, size);
    }

    /// 设置 Legacy 接口的 used ring 对齐。
    pub(crate) fn set_queue_align(&self, align: u32) {
        self.write32(MMIO_QUEUE_ALIGN, align);
    }

    /// 设置 Legacy 接口 queue PFN。
    pub(crate) fn set_queue_pfn(&self, pfn: u32) {
        self.write32(MMIO_QUEUE_PFN, pfn);
    }

    /// 通知设备处理队列。
    pub(crate) fn notify_queue(&self, queue: u32) {
        self.write32(MMIO_QUEUE_NOTIFY, queue);
    }

    /// 清除设备中断状态（本实验轮询为主，仅做寄存器收敛）。
    pub(crate) fn ack_interrupt(&self) {
        let status = self.read32(MMIO_INTERRUPT_STATUS);
        if status != 0 {
            self.write32(MMIO_INTERRUPT_ACK, status);
        }
    }

    fn read32(&self, offset: usize) -> u32 {
        let ptr = (self.base + offset) as *const u32;
        unsafe { read_volatile(ptr) }
    }

    fn write32(&self, offset: usize, value: u32) {
        let ptr = (self.base + offset) as *mut u32;
        unsafe { write_volatile(ptr, value) }
    }

}