﻿use core::ops::Range;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};
use serde::Deserialize;
use serde_device_tree::{
    buildin::{NodeSeq, Reg, StrSeq},
    from_raw_mut, Dtb, DtbPtr,
};
use spin::Once;

pub(crate) struct BoardInfo {
    pub model: Vec<String>,
    pub smp: usize,
    pub memory: Range<usize>,
    pub uart: Range<usize>,
    pub clint: Range<usize>,
    pub test: Range<usize>,
}

static BOARD: Once<BoardInfo> = Once::new();

pub(crate) fn init(opaque: usize) {
    BOARD.call_once(|| {
        let ptr = DtbPtr::from_raw(opaque as _).unwrap();
        let dtb = Dtb::from(ptr).share();
        let t: Tree = from_raw_mut(&dtb).unwrap();

        BoardInfo {
            model: t.model.iter().map(|m| m.to_string()).collect(),
            smp: t.cpus.cpu.len(),
            memory: t
                .memory
                .iter()
                .map(|m| m.deserialize::<Memory>())
                .find(|m| m.device_type.iter().any(|t| t == "memory"))
                .map(|m| m.reg.iter().next().unwrap().0.clone())
                .unwrap(),
            uart: t
                .soc
                .uart
                .iter()
                .map(|u| u.deserialize::<Uart>())
                .find(|u| u.compatible.iter().any(|s| s == "ns16550a"))
                .map(|u| u.reg.iter().next().unwrap().0.clone())
                .unwrap(),
            clint: t
                .soc
                .clint
                .iter()
                .map(|u| u.deserialize::<Clint>())
                .find(|u| u.compatible.iter().any(|s| s == "riscv,clint0"))
                .map(|u| u.reg.iter().next().unwrap().0.clone())
                .unwrap(),
            test: t
                .soc
                .test
                .iter()
                .map(|u| u.deserialize::<Peripheral>())
                .next()
                .map(|u| u.reg.iter().next().unwrap().0.clone())
                .unwrap(),
        }
    });
}

pub(crate) fn get() -> &'static BoardInfo {
    BOARD.wait()
}

#[derive(Deserialize)]
struct Tree<'a> {
    model: StrSeq<'a>,
    cpus: Cpus<'a>,
    memory: NodeSeq<'a>,
    soc: Soc<'a>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Cpus<'a> {
    cpu: NodeSeq<'a>,
}

#[derive(Deserialize)]
struct Soc<'a> {
    uart: NodeSeq<'a>,
    clint: NodeSeq<'a>,
    test: NodeSeq<'a>,
}

#[derive(Deserialize)]
struct Memory<'a> {
    device_type: StrSeq<'a>,
    reg: Reg<'a>,
}

#[derive(Deserialize)]
struct Uart<'a> {
    compatible: StrSeq<'a>,
    reg: Reg<'a>,
}

#[derive(Deserialize)]
struct Clint<'a> {
    compatible: StrSeq<'a>,
    reg: Reg<'a>,
}

#[derive(Deserialize)]
struct Peripheral<'a> {
    reg: Reg<'a>,
}
