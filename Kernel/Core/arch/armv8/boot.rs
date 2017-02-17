// "Tifflin" Kernel
// - By John Hodge (thePowersGang)
//
// Core/arch/armv8/boot.rs
// - ARMv8 (AArch64) Boot Information
use lib::lazy_static::LazyStatic;
use super::fdt::FDTRoot;
use super::memory::addresses::{IDENT_SIZE,IDENT_START};

#[repr(C)]
#[derive(Debug)]
struct SymbolInfo {
	base: *const u8,
	count: usize,
	string_table: *const u8,
	strtab_len: usize,
}

extern "C" {
	static dt_phys_base: u64;
	static kernel_phys_start: u64;
	static symbol_info_phys: u64;
	static ram_first_free: u64;
	static mut kernel_hwmap_level3: [u64; 2048];
	static v_kernel_end: ::Void;
}

enum BootInfo
{
	None,
	Basic(u32,u32),
	FDT(FDTRoot<'static>),
}

static S_BOOT_INFO: LazyStatic<BootInfo> = LazyStatic::new();
static mut S_MEMMAP_DATA: [::memory::MemoryMapEnt; 16] = [::memory::MAP_PAD; 16];

pub fn get_fdt() -> Option<&'static FDTRoot<'static>> {
	match get_boot_info()
	{
	&BootInfo::FDT(ref fdt) => Some(fdt),
	_ => None,
	}
}

fn get_boot_info() -> &'static BootInfo {
	if ! S_BOOT_INFO.ls_is_valid() {
		// SAFE: Shouldn't be called in a racy manner
		unsafe { S_BOOT_INFO.prep(|| BootInfo::new()) }
	}
	&S_BOOT_INFO
}

impl BootInfo
{
	fn new() -> BootInfo {
		// SAFE: Sane extern static
		if unsafe { dt_phys_base } == 0 {
			BootInfo::None
		}
		else {
			// SAFE: In practice, this is run in a single-thread. Any possible race would be benign
			unsafe {
				const FLAGS: u64 = 0x1;
				assert_eq!(kernel_hwmap_level3[1], 0);
				kernel_hwmap_level3[1] = (dt_phys_base & !0x3FFF) + FLAGS;
			}

			// SAFE: Address range checked
			unsafe {
				assert!(symbol_info_phys - kernel_phys_start < IDENT_SIZE as u64,
					"Symbol information {:#x} outside of ident {:#x}+{:#x}", symbol_info_phys, kernel_phys_start, IDENT_SIZE);
				let info: &'static SymbolInfo = &*((symbol_info_phys - kernel_phys_start + IDENT_START as u64) as *const SymbolInfo);
				log_debug!("(symbol) info = {:?}", info);
				if !info.base.is_null() {
					let syms_addr = info.base as usize - kernel_phys_start as usize;
					let strs_addr = info.string_table as usize - kernel_phys_start as usize;
					assert!(syms_addr < IDENT_SIZE);
					assert!(strs_addr < IDENT_SIZE);
					let syms = ::core::slice::from_raw_parts( (syms_addr + IDENT_START) as *const ::symbols::Elf32_Sym, info.count as usize);
					let strs = ::core::slice::from_raw_parts( (strs_addr + IDENT_START) as *const u8, info.strtab_len as usize);
					::symbols::set_symtab(syms, strs, 0);
				}
			}
			
			// SAFE: Memory is valid, and is immutable
			unsafe {
				BootInfo::FDT( super::fdt::FDTRoot::new_raw( (super::memory::addresses::HARDWARE_BASE + 0x4000 + (dt_phys_base & 0x3FFF) as usize) as *const u8 ) )
			}
		}
	}
}

pub fn get_video_mode() -> Option<::metadevs::video::bootvideo::VideoMode> {
	None
}

pub fn get_boot_string() -> &'static str {
	match get_boot_info()
	{
	&BootInfo::FDT(ref fdt) => fdt.get_props(&["","chosen","bootargs"]).next().map(|x| ::core::str::from_utf8(&x[..x.len()-1]).unwrap_or("") ).unwrap_or(""),
	_ => "",
	}
}

pub fn get_memory_map() -> &'static [::memory::MemoryMapEnt] {
	// TODO: Assert that this is only ever called once
	// SAFE: Assuming this function is called only once (which it is)
	let buf: &mut [_] = unsafe { &mut S_MEMMAP_DATA };
	let len = {
		let mut mapbuilder = ::memory::MemoryMapBuilder::new(buf);
		match get_boot_info()
		{
		&BootInfo::None => {},
		&BootInfo::Basic(ram_base, ram_len) => {
			mapbuilder.append( ram_base as u64, ram_len as u64, ::memory::MemoryState::Free, 0 );
			},
		&BootInfo::FDT(ref fdt) => {
			// FDT Present, need to locate all memory nodes
			fdt.dump_nodes();
			for prop in fdt.get_props(&["","memory","reg"])
			{
				use lib::byteorder::{ReadBytesExt,BigEndian};
				let mut p = prop;
				let base = p.read_u64::<BigEndian>().unwrap();
				let size = p.read_u64::<BigEndian>().unwrap();
				log_debug!("base = {:#x}, size = {:#x}", base, size);
				mapbuilder.append( base, size, ::memory::MemoryState::Free, 0 );
			}
			// SAFE: Accesses sane extern static
			mapbuilder.set_range( unsafe { dt_phys_base } as u64, fdt.size() as u64, ::memory::MemoryState::Used, 0 ).unwrap();
			},
		}

		// SAFE: Accesses sane extern statics
		unsafe {
			if kernel_phys_start != 0 {
				// 2. Clobber out kernel, modules, and strings
				mapbuilder.set_range( kernel_phys_start as u64, (&v_kernel_end as *const _ as u64 - 0x80000000), ::memory::MemoryState::Used, 0 ).unwrap();
			}
			if ram_first_free != 0 {
				mapbuilder.set_range( kernel_phys_start as u64, (ram_first_free - kernel_phys_start) as u64, ::memory::MemoryState::Used, 0 ).unwrap();
			}
		}
		
		mapbuilder.size()
		};
	&buf[..len]
}

