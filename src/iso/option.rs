use std::path::PathBuf;
use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(
    name = "mkisofs-rs",
    about = "create an hybrid ISO-9660 filesystem-image with Rock Ridge attributes."
)]
pub struct Opt {
    #[structopt(long, short = "o", help = "Set output file name")]
    pub output: String,

    #[structopt(flatten)]
    pub eltorito_opt: ElToritoOpt,

    #[structopt(
        long = "generic-boot",
        short = "G",
        help = "Copy at most 32768 bytes from the given disk file to the very start of the ISO image",
        aliases = &["embedded-boot"]
    )]
    pub embedded_boot: Option<String>,

    #[structopt(
        long = "grub2-mbr",
        help = "Patch and embedded_boot to simplify hybrid images"
    )]
    pub grub2_mbr: Option<String>,

    #[structopt(
        long = "boot-load-size",
        help = "Set the number of 512-byte blocks to be loaded at boot time from the boot image in the current catalog entry.",
        default_value = "4"
    )]
    pub boot_load_size: u32,

    #[structopt(
        long = "protective-msdos-label",
        help = "Patch the System Area by a simple PC-DOS partition table where partition 1 claims the range of the ISO image but leaves the first block unclaimed."
    )]
    pub protective_msdos_label: bool,

    #[structopt(parse(from_os_str))]
    pub input_files: Vec<PathBuf>,
}

#[derive(StructOpt, Debug)]
pub struct ElToritoOpt {
    #[structopt(
        long = "eltorito-boot",
        short = "b",
        help = "Set El Torito boot image name"
    )]
    pub eltorito_boot: Option<String>,

    #[structopt(long = "no-emul-boot", help = "Boot image is 'no emulation' image")]
    pub no_emu_boot: bool,

    #[structopt(long = "no-boot", help = "Boot image is not bootable")]
    pub no_boot: bool,

    #[structopt(long = "boot-info-table", help = "Patch boot image with info table")]
    pub boot_info_table: bool,

    #[structopt(long = "grub2-boot-info", help = "Patch for GRUB 2 El Torino image")]
    pub grub2_boot_info: bool,
}
