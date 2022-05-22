use crate::{clint, hart_id, Supervisor};

#[repr(C)]
#[derive(Debug)]
struct Context {
    msp: usize,
    x: [usize; 31],
    mstatus: usize,
    mepc: usize,
}

pub(crate) fn execute_supervisor(supervisor: Supervisor) {
    use core::arch::asm;
    use riscv::register::{medeleg, mie, mip, mstatus};

    unsafe {
        mstatus::set_mpp(mstatus::MPP::Supervisor);
        mstatus::set_mie();
    };

    let mut ctx = Context {
        msp: 0,
        x: [0; 31],
        mstatus: 0,
        mepc: supervisor.start_addr,
    };

    *ctx.a_mut(0) = hart_id();
    *ctx.a_mut(1) = supervisor.opaque;

    clint::get().clear_soft(hart_id());
    unsafe {
        asm!("csrr {}, mstatus", out(reg) ctx.mstatus);
        asm!("csrw     mip, {}", in(reg) 0);
        asm!("csrw mideleg, {}", in(reg) usize::MAX);
        asm!("csrw medeleg, {}", in(reg) usize::MAX);
        mstatus::clear_mie();
        medeleg::clear_illegal_instruction();
        medeleg::clear_supervisor_env_call();
        medeleg::clear_machine_env_call();

        crate::set_mtcev(s_to_m as usize);
        mie::set_mext();
        mie::set_msoft();
    }

    loop {
        use crate::qemu_hsm::{EID_HSM, FID_HART_STOP, FID_HART_SUSPEND, SUSPEND_NON_RETENTIVE};
        use riscv::register::mcause::{self, Exception as E, Interrupt as I, Trap as T};

        unsafe { m_to_s(&mut ctx) };

        match mcause::read().cause() {
            T::Exception(E::SupervisorEnvCall) => {
                let param = [ctx.a(0), ctx.a(1), ctx.a(2), ctx.a(3), ctx.a(4), ctx.a(5)];
                let ans = rustsbi::ecall(ctx.a(7), ctx.a(6), param);
                if ctx.a(7) == EID_HSM && ans.error == 0 {
                    if ctx.a(6) == FID_HART_STOP {
                        return;
                    }
                    if ctx.a(6) == FID_HART_SUSPEND && ctx.a(0) == SUSPEND_NON_RETENTIVE as usize {
                        return;
                    }
                }
                *ctx.a_mut(0) = ans.error;
                *ctx.a_mut(1) = ans.value;
                ctx.mepc = ctx.mepc.wrapping_add(4);
            }
            T::Interrupt(I::MachineTimer) => unsafe {
                mip::clear_mtimer();
                mip::set_stimer();
            },
            T::Exception(E::IllegalInstruction) => {
                println!("TODO emulate or forward illegal instruction");
                break;
            }
            t => {
                println!("{t:?}");
                break;
            }
        }
    }
    loop {
        core::hint::spin_loop();
    }
}

impl Context {
    #[inline]
    fn a(&self, n: usize) -> usize {
        self.x[9 + n]
    }

    #[inline]
    fn a_mut(&mut self, n: usize) -> &mut usize {
        &mut self.x[9 + n]
    }
}

/// M 态转到 S 态。
///
/// # Safety
///
/// 裸函数，手动保存所有上下文环境。
/// 为了写起来简单，占 32 * usize 空间，循环 31 次保存 31 个通用寄存器。
/// 实际 x0(zero) 和 x2(sp) 不需要保存在这里。
#[naked]
unsafe extern "C" fn m_to_s(ctx: &mut Context) {
    core::arch::asm!(
        r"
        .altmacro
        .macro SAVE_M n
            sd x\n, \n*8(sp)
        .endm
        .macro LOAD_S n
            ld x\n, \n*8(sp)
        .endm
        ",
        // 入栈
        "
        addi sp, sp, -32*8
        ",
        // 保存 x[1..31]
        "
        .set n, 1
        .rept 31
            SAVE_M %n
            .set n, n+1
        .endr
        ",
        // M sp 保存到 S ctx
        "
        sd sp, 0(a0)
        mv sp, a0
        ",
        // 利用 tx 恢复 csr
        // S ctx.x[2](sp) => mscratch
        // S ctx.mstatus  => mstatus
        // S ctx.mepc     => mepc
        "
        ld   t0,  2*8(sp)
        ld   t1, 32*8(sp)
        ld   t2, 33*8(sp)
        csrw mscratch, t0
        csrw  mstatus, t1
        csrw     mepc, t2
        ",
        // 从 S ctx 恢复 x[1,3..32]
        "
        ld x1, 1*8(sp)
        .set n, 3
        .rept 29
            LOAD_S %n
            .set n, n+1
        .endr
        ",
        // 换栈：
        // sp      : S sp
        // mscratch: S ctx
        "
        csrrw sp, mscratch, sp
        mret
        ",
        options(noreturn)
    )
}

/// S 态陷入 M 态。
///
/// # Safety
///
/// 裸函数。
/// 利用恢复的 ra 回到 [`m_to_s`] 的返回地址。
#[naked]
#[link_section = ".text.trap_handler"]
unsafe extern "C" fn s_to_m() {
    core::arch::asm!(
        r"
        .altmacro
        .macro SAVE_S n
            sd x\n, \n*8(sp)
        .endm
        .macro LOAD_M n
            ld x\n, \n*8(sp)
        .endm
        ",
        // 换栈：
        // sp      : S ctx
        // mscratch: S sp
        "
        csrrw sp, mscratch, sp
        ",
        // 保存 x[1,3..32] 到 S ctx
        "
        sd x1, 1*8(sp)
        .set n, 3
        .rept 29
            SAVE_S %n
            .set n, n+1
        .endr
        ",
        // 利用 tx 保存 csr
        // mscratch => S ctx.x[2](sp)
        // mstatus  => S ctx.mstatus
        // mepc     => S ctx.mepc
        "
        csrr t0, mscratch
        csrr t1, mstatus
        csrr t2, mepc
        sd   t0,  2*8(sp)
        sd   t1, 32*8(sp)
        sd   t2, 33*8(sp)
        ",
        // 从 S ctx 恢复 M sp
        "
        ld sp, 0(sp)
        ",
        // 恢复 s[0..12]
        "
        .set n, 1
        .rept 31
            LOAD_M %n
            .set n, n+1
        .endr
        ",
        // 出栈完成，栈指针归位
        // 返回
        "
        addi sp, sp, 32*8
        ret
        ",
        options(noreturn)
    )
}
