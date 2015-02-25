//
//
//
//! ATA IO code, handling device multiplexing and IO operations
use kernel::_common::*;
use kernel::memory::helpers::{DMABuffer};
use kernel::async::EventWait;
use kernel::device_manager::IOBinding;

pub const SECTOR_SIZE: usize = 512;
const MAX_DMA_SECTORS: usize = 0x10000 / SECTOR_SIZE;	// Limited by byte count, 16-9 = 7 bits = 128 sectors

//const HDD_PIO_W28: u8 = 0x30,
//const HDD_PIO_R28: u8 = 0x20;
//const HDD_PIO_W48: u8 = 0x34;
//const HDD_PIO_R48: u8 = 0x24,
//const HDD_IDENTIFY: u8 = 0xEC

const HDD_DMA_R28: u8 = 0xC8;
const HDD_DMA_W28: u8 = 0xCA;
const HDD_DMA_R48: u8 = 0x25;
const HDD_DMA_W48: u8 = 0x35;

pub struct DmaController
{
	pub ata_controllers: [AtaController; 2],
	pub dma_base: IOBinding,
}
struct DmaRegBorrow<'a>
{
	dma_base: &'a IOBinding,
	is_sec: bool,
}
pub struct AtaController
{
	regs: ::kernel::async::Mutex<AtaRegs>,
	interrupt: AtaInterrupt,
}
struct AtaRegs
{
	ata_base: u16,
	sts_base: u16,
	prdts: ::kernel::memory::virt::ArrayHandle<PRDTEnt>,
}
struct AtaInterrupt
{
	handle: ::kernel::irqs::Handle,
	event: ::kernel::async::EventSource,
}

#[repr(C)]
struct PRDTEnt
{
	addr: u32,
	bytes: u16,
	flags: u16,
}

impl DmaController
{
	pub fn do_dma<'s>(&'s self, blockidx: u64, count: usize, dst: &'s [u8], disk: u8, is_write: bool) -> Result<EventWait<'s>,()>
	{
		assert!(disk < 4);
		assert!(count < MAX_DMA_SECTORS);
		assert_eq!(dst.len(), count * SECTOR_SIZE);
		
		let bus = (disk >> 1) & 1;
		let disk = disk & 1;
		
		// Try to obtain a DMA context
		Ok( self.ata_controllers[bus as usize].do_dma(blockidx, dst, disk, is_write, DmaRegBorrow::new(self, bus == 1) ) )
	}
}

impl<'a> DmaRegBorrow<'a>
{
	fn new(dm: &DmaController, is_secondary: bool) -> DmaRegBorrow
	{
		DmaRegBorrow {
			dma_base: &dm.dma_base,
			is_sec: is_secondary,
		}
	}
	
	unsafe fn out_32(&self, ofs: u16, val: u32)
	{
		assert!(ofs < 8);
		self.dma_base.write_32( if self.is_sec { 8 } else { 0 } + ofs as usize, val );
		unimplemented!();
	}
	unsafe fn out_8(&self, ofs: u16, val: u8)
	{
		assert!(ofs < 8);
		self.dma_base.write_8( if self.is_sec { 8 } else { 0 } + ofs as usize, val );
	}
	
}

impl AtaRegs
{
	fn new(ata_base: u16, sts_port: u16) -> AtaRegs
	{
		AtaRegs {
			ata_base: ata_base, sts_base: sts_port,
			prdts: ::kernel::memory::virt::alloc_dma(32, 1, module_path!()).unwrap().into_array(),
		}
	}
	
	unsafe fn out_8(&self, ofs: u16, val: u8)
	{
		::kernel::arch::x86_io::outb( self.ata_base + ofs, val);
	}
	
	fn start_dma(&mut self, disk: u8, blockidx: u64, dma_buffer: &DMABuffer, is_write: bool, bm: DmaRegBorrow)
	{
		let count = dma_buffer.len() / SECTOR_SIZE;
		// Fill PRDT
		// TODO: Use a chain of PRDTs to support 32-bit scatter-gather
		self.prdts[0].bytes = dma_buffer.len() as u16;
		self.prdts[0].addr = dma_buffer.phys() as u32;
		
		// Commence the IO and return a wait handle for the operation
		unsafe
		{
			// - Only use LBA48 if needed
			if blockidx >= (1 << 28)
			{
				self.out_8(6, 0x40 | (disk << 4));
				self.out_8(2, 0);	// Upper sector count (must be zero because of MAX_DMA_SECTORS)
				self.out_8(3, (blockidx >> 24) as u8);
				self.out_8(4, (blockidx >> 32) as u8);
				self.out_8(5, (blockidx >> 40) as u8);
			}
			else
			{
				self.out_8(6, 0xE0 | (disk << 4) | ((blockidx >> 24) & 0x0F) as u8);
			}
			self.out_8(2, count as u8);
			self.out_8(3, (blockidx >>  0) as u8);
			self.out_8(4, (blockidx >>  8) as u8);
			self.out_8(5, (blockidx >> 16) as u8);
			
			// - Set PRDT
			bm.out_32(4, ::kernel::memory::virt::get_phys(&self.prdts[0]) as u32);
			bm.out_8(0, 0x04);	// Reset IRQ
			
			self.out_8(7,
				if blockidx >= (1 << 48) {
					if is_write { HDD_DMA_W48 } else { HDD_DMA_R48 }	// LBA 48
				} else {
					if is_write { HDD_DMA_W28 } else { HDD_DMA_R28 }	// LBA 28
				});
			
			// Start IO
			bm.out_8(0, if is_write { 0 } else { 8 } | 1);
		}
	}
}

impl AtaController
{
	pub fn new(ata_base: u16, sts_port: u16, irq: u32) -> AtaController
	{
		AtaController {
			regs: ::kernel::async::Mutex::new( AtaRegs::new(ata_base, sts_port) ),
			interrupt: AtaInterrupt {
				handle: ::kernel::irqs::bind_interrupt_event(irq),
				event: ::kernel::async::EventSource::new(),
				},
			}
	}
	
	fn wait_handle<'a, F: FnOnce(&mut EventWait) + Send + 'a> (&'a self, f: F) -> EventWait<'a>
	{
		self.interrupt.event.wait_on(f)
	}
	
	fn do_dma<'a>(&'a self, blockidx: u64, dst: &'a [u8], disk: u8, is_write: bool, dma_regs: DmaRegBorrow) -> EventWait<'a>
	{
		if let Some(mut buslock) = self.regs.try_lock()
		{
			let dma_buffer = DMABuffer::new_contig( unsafe { ::core::mem::transmute(dst) }, 32 );
			buslock.start_dma( disk, blockidx, &dma_buffer, is_write, dma_regs );
			
			self.wait_handle( |_| { drop(dma_buffer); drop(buslock) } )
		}
		else
		{
			unimplemented!();
			// If obtaining a context failed, put the request on the queue and return a wait handle for it
			/*
			self.regs.async_lock(|event_ref, mut buslock| {
				let dma_buffer = DMABuffer::new_contig( unsafe { ::core::mem::transmute(dst) }, 32 );
				buslock.start_dma(disk, blockidx, &dma_buffer, is_write, dma_regs);
				*event_ref = self.wait_handle( |_| { drop(dma_buffer); drop(buslock) });
				})
			*/
		}
	}
}

