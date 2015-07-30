// Tifflin OS - simple_console
// - By John Hodge (thePowersGang)
//
// Simplistic console, used as a quick test case (fullscreen window)
#![feature(core_slice_ext)]

#[macro_use]
extern crate syscalls;

use syscalls::Object;

mod terminal_surface;

fn main() {
	use syscalls::gui::{Group,Window};
	use syscalls::threads::S_THIS_PROCESS;
	
	::syscalls::threads::wait(&mut [S_THIS_PROCESS.get_wait()], !0);
	::syscalls::gui::set_group( S_THIS_PROCESS.receive_object::<Group>(0).unwrap() );
	
	let window = Window::new("Console").unwrap();
	window.maximise();
	window.fill_rect(0,0, !0,!0, 0x33_00_00);   // A nice rust-like red :)
	let mut term = terminal_surface::Surface::new(&window, ::syscalls::gui::Rect::new(0,0, 1920,1080));
	term.putc(0, ::syscalls::gui::Colour::white(), 'H');
	window.show();
	
	loop {
		// Bind to receive events relating to the window
		let mut events = [window.get_wait()];
		
		::syscalls::threads::wait(&mut events, !0);
	
		while let Some(ev) = window.pop_event()
		{
			kernel_log!("ev = {:?}", ev);
		}
		
		window.check_wait(&events[0]);
	}
}

