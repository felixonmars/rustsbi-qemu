use std::{
    env,
    path::{Path, PathBuf},
    process::{self, Command},
};

#[macro_use]
extern crate clap;

const DEFAULT_TARGET: &'static str = "riscv64imac-unknown-none-elf";

#[derive(Debug)]
struct XtaskEnv {
    compile_mode: CompileMode,
}

#[derive(Debug)]
enum CompileMode {
    Debug,
    Release
}

fn main() {    
    let matches = clap_app!(xtask =>
        (version: crate_version!())
        (author: crate_authors!())
        (about: crate_description!())
        (@subcommand make =>
            (about: "Build project")
            (@arg release: --release "Build artifacts in release mode, with optimizations")
        )
        (@subcommand asm =>
            (about: "View asm code for project")
        )
        (@subcommand size =>
            (about: "View size for project")
        )
        (@subcommand qemu =>
            (about: "Run QEMU")
            (@arg release: --release "Build artifacts in release mode, with optimizations")
        )
        (@subcommand debug =>
            (about: "Debug with QEMU and GDB stub")
        )
        (@subcommand gdb =>
            (about: "Run GDB debugger")
        )
    ).get_matches();
    let mut xtask_env = XtaskEnv {
        compile_mode: CompileMode::Debug,
    };
    println!("xtask: mode: {:?}", xtask_env.compile_mode);
    if let Some(matches) = matches.subcommand_matches("make") {
        if matches.is_present("release") {
            xtask_env.compile_mode = CompileMode::Release;
        }
        xtask_build_sbi(&xtask_env);
        xtask_binary_sbi(&xtask_env);
        xtask_build_test_kernel(&xtask_env);
        xtask_binary_test_kernel(&xtask_env);
    } else if let Some(matches) = matches.subcommand_matches("qemu") {
        if matches.is_present("release") {
            xtask_env.compile_mode = CompileMode::Release;
        }
        xtask_build_sbi(&xtask_env);
        xtask_binary_sbi(&xtask_env);
        xtask_build_test_kernel(&xtask_env);
        xtask_binary_test_kernel(&xtask_env);
        xtask_qemu_run(&xtask_env);
    }  else if let Some(_matches) = matches.subcommand_matches("debug") {
        xtask_build_sbi(&xtask_env);
        xtask_binary_sbi(&xtask_env);
        xtask_build_test_kernel(&xtask_env);
        xtask_binary_test_kernel(&xtask_env);
        xtask_qemu_debug(&xtask_env);
    } else if let Some(_matches) = matches.subcommand_matches("asm") {
        xtask_build_sbi(&xtask_env);
        xtask_asm_sbi(&xtask_env);
    } else if let Some(_matches) = matches.subcommand_matches("size") {
        xtask_build_sbi(&xtask_env);
        xtask_size_sbi(&xtask_env);
    } else if let Some(_matches) = matches.subcommand_matches("gdb") {
        xtask_gdb(&xtask_env);
    } else {
        println!("Use `cargo qemu` to run, `cargo xtask --help` for help")
    }
}

fn xtask_build_sbi(xtask_env: &XtaskEnv) {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command.current_dir(project_root().join("rustsbi-qemu"));
    command.arg("build");
    match xtask_env.compile_mode {
        CompileMode::Debug => {},
        CompileMode::Release => { command.arg("--release"); },
    }
    command.args(&["--package", "rustsbi-qemu"]);
    command.args(&["--target", DEFAULT_TARGET]);
    let status = command
        .status().unwrap();
    if !status.success() {
        println!("cargo build failed");
        process::exit(1);
    }
}

fn xtask_build_test_kernel(xtask_env: &XtaskEnv) {
    let cargo = env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut command = Command::new(cargo);
    command.current_dir(project_root().join("test-kernel"));
    command.arg("build");
    match xtask_env.compile_mode {
        CompileMode::Debug => {},
        CompileMode::Release => { command.arg("--release"); },
    }
    command.args(&["--package", "test-kernel"]);
    command.args(&["--target", DEFAULT_TARGET]);
    let status = command
        .status().unwrap();
    if !status.success() {
        println!("cargo build failed");
        process::exit(1);
    }
}

fn xtask_asm_sbi(xtask_env: &XtaskEnv) {
    // @{{objdump}} -D {{test-kernel-elf}} | less
    let objdump = "rust-objdump";
    Command::new(objdump)
        .current_dir(dist_dir(xtask_env))
        .arg("-d")
        .arg("rustsbi-qemu")
        .status().unwrap();
}

fn xtask_size_sbi(xtask_env: &XtaskEnv) {
    // @{{size}} -A -x {{test-kernel-elf}} 
    let size = "rust-size";
    Command::new(size)
        .current_dir(dist_dir(xtask_env))
        .arg("-A")
        .arg("-x")
        .arg("rustsbi-qemu")
        .status().unwrap();
}

fn xtask_binary_sbi(xtask_env: &XtaskEnv) {
    /*
    objdump := "riscv64-unknown-elf-objdump"
objcopy := "rust-objcopy --binary-architecture=riscv64"

build: firmware
    @{{objcopy}} {{test-kernel-elf}} --strip-all -O binary {{test-kernel-bin}}
 */
    let objcopy = "rust-objcopy";
    let status = Command::new(objcopy)
        .current_dir(dist_dir(xtask_env))
        .arg("rustsbi-qemu")
        .arg("--binary-architecture=riscv64")
        .arg("--strip-all")
        .args(&["-O", "binary", "rustsbi-qemu.bin"])
        .status().unwrap();

    if !status.success() {
        println!("objcopy binary failed");
        process::exit(1);
    }
}

fn xtask_binary_test_kernel(xtask_env: &XtaskEnv) {
    let objcopy = "rust-objcopy";
    let status = Command::new(objcopy)
        .current_dir(dist_dir(xtask_env))
        .arg("test-kernel")
        .arg("--binary-architecture=riscv64")
        .arg("--strip-all")
        .args(&["-O", "binary", "test-kernel.bin"])
        .status().unwrap();

    if !status.success() {
        println!("objcopy binary failed");
        process::exit(1);
    }
}

fn xtask_qemu_run(xtask_env: &XtaskEnv) {
    /*
    qemu: build
    @qemu-system-riscv64 \
            -machine virt \
            -nographic \
            -bios none \
            -device loader,file={{rustsbi-bin}},addr=0x80000000 \
            -device loader,file={{test-kernel-bin}},addr=0x80200000 \
            -smp threads={{threads}}
    */
    let status = Command::new("qemu-system-riscv64")
        .current_dir(dist_dir(xtask_env))
        .args(&["-machine", "virt"])
        .args(&["-bios", "rustsbi-qemu.bin"])
        .arg("-nographic")
        .args(&["-device", "loader,file=test-kernel.bin,addr=0x80200000"])
        .status().unwrap();
    
    if !status.success() {
        println!("qemu failed");
        process::exit(1);
    }
}

fn xtask_qemu_debug(xtask_env: &XtaskEnv) {
    let status = Command::new("qemu-system-riscv64")
        .current_dir(dist_dir(xtask_env))
        .args(&["-machine", "virt"])
        .args(&["-bios", "rustsbi-qemu.bin"])
        .arg("-nographic")
        .args(&["-device", "loader,file=test-kernel.bin,addr=0x80200000"])
        .args(&["-gdb", "tcp::1234", "-S"])
        .status().unwrap();
    
    if !status.success() {
        println!("qemu failed");
        process::exit(1);
    }
}

fn xtask_gdb(xtask_env: &XtaskEnv) {
    let status = Command::new("riscv64-unknown-elf-gdb")
        .current_dir(dist_dir(xtask_env))
        .args(&["--eval-command", "file rustsbi-qemu"])
        .args(&["--eval-command", "target remote localhost:1234"])
        .arg("-q")
        .status().unwrap();
    
    if !status.success() {
        println!("qemu failed");
        process::exit(1);
    }
}

fn project_root() -> PathBuf {
    Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

fn dist_dir(xtask_env: &XtaskEnv) -> PathBuf {
    let mut path_buf = project_root().join("target").join(DEFAULT_TARGET);
    path_buf = match xtask_env.compile_mode {
        CompileMode::Debug => path_buf.join("debug"),
        CompileMode::Release => path_buf.join("release"),
    };
    path_buf
}