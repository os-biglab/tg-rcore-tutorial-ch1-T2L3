//! VirtIO 块设备最小驱动（仅支持同步读写一个 512B 扇区）。

use core::ptr::{addr_of, addr_of_mut};

use crate::{
    virtio_mmio::{
        STATUS_ACKNOWLEDGE, STATUS_DRIVER, STATUS_DRIVER_OK, STATUS_FAILED, STATUS_FEATURES_OK,
        VIRTIO0, VirtioMmio,
    },
    virtqueue::{QUEUE_SIZE, VirtQueue},
};

/// 扇区大小（字节）。
pub(crate) const BLK_SIZE: usize = 512;
const PAGE_SIZE: usize = 4096;

const REQ_TYPE_IN: u32 = 0;
const REQ_TYPE_OUT: u32 = 1;
const VIRTIO_BLK_S_OK: u8 = 0;

/// 块请求头。
#[repr(C)]
pub(crate) struct BlkReqHeader {
    type_: u32,
    reserved: u32,
    sector: u64,
}

/// VirtIO 块设备错误。
#[derive(Copy, Clone)]
pub(crate) enum BlkError {
    /// MMIO 初始化错误。
    Mmio(&'static str),
    /// 队列配置错误。
    Queue(&'static str),
    /// 设备返回 I/O 错误状态。
    Io(u8),
}

/// 简化版 VirtIO 块设备。
pub(crate) struct VirtIOBlk {
    mmio: VirtioMmio,
    queue: VirtQueue,
}

impl VirtIOBlk {
    /// 初始化一个 VirtIO 块设备（queue 0）。
    pub(crate) fn new() -> Result<Self, BlkError> {
        let mmio = VirtioMmio::new(VIRTIO0);
        mmio.probe_block_device().map_err(BlkError::Mmio)?;

        mmio.set_status(0);
        mmio.add_status(STATUS_ACKNOWLEDGE);
        mmio.add_status(STATUS_DRIVER);

        let _features_lo = mmio.read_device_features(0);
        let _features_hi = mmio.read_device_features(1);
        mmio.write_driver_features(0, 0);
        mmio.write_driver_features(1, 0);

        mmio.add_status(STATUS_FEATURES_OK);
        if mmio.status() & STATUS_FEATURES_OK == 0 {
            mmio.add_status(STATUS_FAILED);
            return Err(BlkError::Mmio("features not accepted"));
        }

        mmio.set_queue_sel(0);
        let max = mmio.queue_num_max();
        if max == 0 {
            mmio.add_status(STATUS_FAILED);
            return Err(BlkError::Queue("queue 0 is unavailable"));
        }
        if max < QUEUE_SIZE as u32 {
            mmio.add_status(STATUS_FAILED);
            return Err(BlkError::Queue("queue size too small"));
        }

        let queue = VirtQueue::new();
        mmio.set_queue_num(QUEUE_SIZE as u32);
        mmio.set_guest_page_size(PAGE_SIZE as u32);
        mmio.set_queue_align(PAGE_SIZE as u32);
        let queue_paddr = queue.desc_paddr();
        if queue_paddr & (PAGE_SIZE - 1) != 0 {
            mmio.add_status(STATUS_FAILED);
            return Err(BlkError::Queue("legacy queue address is not page aligned"));
        }
        mmio.set_queue_pfn((queue_paddr / PAGE_SIZE) as u32);
        mmio.add_status(STATUS_DRIVER_OK);

        Ok(Self { mmio, queue })
    }

    /// 读取一个扇区到 `buf`。
    pub(crate) fn read_block(
        &mut self,
        sector: u64,
        buf: &mut [u8; BLK_SIZE],
    ) -> Result<(), BlkError> {
        let header = BlkReqHeader {
            type_: REQ_TYPE_IN,
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0xff;
        self.queue.submit_and_wait(
            addr_of!(header) as usize,
            buf.as_ptr() as usize,
            BLK_SIZE,
            addr_of_mut!(status) as usize,
            true,
        );
        self.mmio.notify_queue(0);
        self.queue.wait_used();
        self.mmio.ack_interrupt();

        if status == VIRTIO_BLK_S_OK {
            Ok(())
        } else {
            Err(BlkError::Io(status))
        }
    }

    /// 将 `buf` 写入一个扇区。
    pub(crate) fn write_block(&mut self, sector: u64, buf: &[u8; BLK_SIZE]) -> Result<(), BlkError> {
        let header = BlkReqHeader {
            type_: REQ_TYPE_OUT,
            reserved: 0,
            sector,
        };
        let mut status: u8 = 0xff;
        self.queue.submit_and_wait(
            addr_of!(header) as usize,
            buf.as_ptr() as usize,
            BLK_SIZE,
            addr_of_mut!(status) as usize,
            false,
        );
        self.mmio.notify_queue(0);
        self.queue.wait_used();
        self.mmio.ack_interrupt();

        if status == VIRTIO_BLK_S_OK {
            Ok(())
        } else {
            Err(BlkError::Io(status))
        }
    }
}