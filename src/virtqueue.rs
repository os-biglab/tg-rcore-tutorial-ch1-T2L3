//! VirtQueue 最小实现（单请求同步轮询）。

use core::{
    hint::spin_loop,
    mem::size_of,
    ptr::{addr_of, addr_of_mut, read_volatile, write_volatile},
    sync::atomic::{Ordering, compiler_fence},
};

/// VirtQueue 长度（描述符个数）。
pub(crate) const QUEUE_SIZE: u16 = 8;
const PAGE_SIZE: usize = 4096;

const DESC_TABLE_SIZE: usize = 16 * (QUEUE_SIZE as usize);
const AVAIL_RING_SIZE: usize = 6 + 2 * (QUEUE_SIZE as usize);
const AVAIL_OFFSET: usize = DESC_TABLE_SIZE;
const USED_OFFSET: usize = (AVAIL_OFFSET + AVAIL_RING_SIZE + PAGE_SIZE - 1) & !(PAGE_SIZE - 1);
const USED_PADDING_SIZE: usize = USED_OFFSET - (AVAIL_OFFSET + AVAIL_RING_SIZE);

const DESC_F_NEXT: u16 = 1;
const DESC_F_WRITE: u16 = 2;

/// 描述符。
#[repr(C)]
#[derive(Copy, Clone)]
struct Descriptor {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

const DESC_ZERO: Descriptor = Descriptor {
    addr: 0,
    len: 0,
    flags: 0,
    next: 0,
};

/// 已完成队列元素。
#[repr(C)]
#[derive(Copy, Clone)]
struct UsedElem {
    id: u32,
    len: u32,
}

const USED_ELEM_ZERO: UsedElem = UsedElem { id: 0, len: 0 };

/// 可用环。
#[repr(C)]
struct AvailRing {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE as usize],
    used_event: u16,
}

/// 已用环。
#[repr(C)]
struct UsedRing {
    flags: u16,
    idx: u16,
    ring: [UsedElem; QUEUE_SIZE as usize],
    avail_event: u16,
}

/// 队列内存区域。
#[repr(C, align(4096))]
struct QueueMem {
    desc: [Descriptor; QUEUE_SIZE as usize],
    avail: AvailRing,
    used_padding: [u8; USED_PADDING_SIZE],
    used: UsedRing,
}

static mut QUEUE_MEM: QueueMem = QueueMem {
    desc: [DESC_ZERO; QUEUE_SIZE as usize],
    avail: AvailRing {
        flags: 0,
        idx: 0,
        ring: [0; QUEUE_SIZE as usize],
        used_event: 0,
    },
    used_padding: [0; USED_PADDING_SIZE],
    used: UsedRing {
        flags: 0,
        idx: 0,
        ring: [USED_ELEM_ZERO; QUEUE_SIZE as usize],
        avail_event: 0,
    },
};

/// 单请求同步 VirtQueue。
pub(crate) struct VirtQueue {
    mem: *mut QueueMem,
    last_used_idx: u16,
}

impl VirtQueue {
    /// 创建队列。
    pub(crate) fn new() -> Self {
        let mem = addr_of_mut!(QUEUE_MEM);
        unsafe {
            write_volatile(addr_of_mut!((*mem).avail.idx), 0);
            write_volatile(addr_of_mut!((*mem).used.idx), 0);
        }
        Self {
            mem,
            last_used_idx: 0,
        }
    }

    /// Descriptor Table 地址。
    pub(crate) fn desc_paddr(&self) -> usize {
        unsafe { addr_of!((*self.mem).desc) as usize }
    }

    /// 提交一个块请求并阻塞等待完成。
    pub(crate) fn submit_and_wait(
        &mut self,
        header_paddr: usize,
        data_paddr: usize,
        data_len: usize,
        status_paddr: usize,
        device_writes_data: bool,
    ) {
        unsafe {
            let desc = &mut (*self.mem).desc;
            desc[0] = Descriptor {
                addr: header_paddr as u64,
                len: size_of::<super::virtio_blk::BlkReqHeader>() as u32,
                flags: DESC_F_NEXT,
                next: 1,
            };
            desc[1] = Descriptor {
                addr: data_paddr as u64,
                len: data_len as u32,
                flags: if device_writes_data {
                    DESC_F_WRITE | DESC_F_NEXT
                } else {
                    DESC_F_NEXT
                },
                next: 2,
            };
            desc[2] = Descriptor {
                addr: status_paddr as u64,
                len: 1,
                flags: DESC_F_WRITE,
                next: 0,
            };

            let avail_idx = read_volatile(addr_of!((*self.mem).avail.idx));
            let ring_pos = (avail_idx as usize) % (QUEUE_SIZE as usize);
            write_volatile(addr_of_mut!((*self.mem).avail.ring[ring_pos]), 0);

            compiler_fence(Ordering::Release);
            write_volatile(addr_of_mut!((*self.mem).avail.idx), avail_idx.wrapping_add(1));
        }
    }

    /// 轮询等待一个请求完成。
    pub(crate) fn wait_used(&mut self) {
        loop {
            let used_idx = unsafe { read_volatile(addr_of!((*self.mem).used.idx)) };
            if used_idx != self.last_used_idx {
                self.last_used_idx = self.last_used_idx.wrapping_add(1);
                compiler_fence(Ordering::Acquire);
                break;
            }
            spin_loop();
        }
    }
}