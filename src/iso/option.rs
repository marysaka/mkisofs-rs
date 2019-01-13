use structopt::StructOpt;

/// A basic example
#[derive(StructOpt, Debug)]
#[structopt(name = "mkisofs-rs", about = "create an hybrid ISO-9660 filesystem-image with optional Rock Ridge attributes.")]
pub struct Opt
{
    #[structopt(long, short = "o", help = "Set output file name")]
    pub output : String,

    #[structopt(long = "eltorito-boot", short = "b", help = "Set El Torito boot image name (TOOD)")]
    pub eltorito_boot : Option<String>,

    #[structopt(flatten)]
    eltorito_opt: ElToritoOpt,

    pub input_directory: String,
}

#[derive(StructOpt, Debug)]
pub struct ElToritoOpt
{
    #[structopt(long = "no-emul-boot", help = "Boot image is 'no emulation' image (TODO)")]
    no_emu_boot: bool,

    #[structopt(long = "generic-boot", short="G", help = "Copy at most 32768 bytes from the given disk file to the very start of the ISO image (TODO)", raw(aliases = r#"&["embedded-boot"]"#, next_line_help = "true"))]
    embedded_boot: Option<String>,

    #[structopt(long = "no-boot", help = "Boot image is not bootable (TODO)")]
    no_boot: bool,

    //#[structopt(long = "hard-disk-boot", help = "Boot image is a hard disk image (Unsupported)")]
    //hard_disk_boot: bool,
    
    #[structopt(long = "boot-info-table", help = "Patch boot image with info table (TODO)")]
    boot_info_table: bool,
}