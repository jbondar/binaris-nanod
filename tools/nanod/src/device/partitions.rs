use crate::device::constants::*;

pub struct PartitionInfo {
    pub name: &'static str,
    pub offset: u32,
    pub size: u32,
}

pub const PARTITIONS: &[PartitionInfo] = &[
    PartitionInfo {
        name: "nvs",
        offset: 0x9000,
        size: 0x5000,
    },
    PartitionInfo {
        name: "otadata",
        offset: OTADATA_OFFSET,
        size: OTADATA_SIZE,
    },
    PartitionInfo {
        name: "app0",
        offset: APP0_OFFSET,
        size: APP0_SIZE,
    },
    PartitionInfo {
        name: "app1",
        offset: 0x150000,
        size: 0x140000,
    },
    PartitionInfo {
        name: "spiffs",
        offset: 0x290000,
        size: 0x160000,
    },
    PartitionInfo {
        name: "coredump",
        offset: 0x3F0000,
        size: 0x10000,
    },
];
