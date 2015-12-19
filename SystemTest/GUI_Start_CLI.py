import TestInstance
import sys
import time

from TestInstance import test_assert

KEYNAME_MAP = {
    'meta_l': "LeftGui",
    'ret': "Return",
    'down': "DownArrow",
    'up': "UpArrow",
    
    'alt': "LeftAlt",
    'f4': "F4",
    
    'e': "E",
    't': "T",
    }

def _keypress(instance, key, name, idle=True):
    timeout = 2.0
    if isinstance(key, list):
        instance.type_combo(key)
        for k in key:
            test_assert(name+" "+k+" press timeout", instance.wait_for_line("\[syscalls\] - USER> (Window|Menu)::handle_event\(ev=KeyDown\("+KEYNAME_MAP[k]+"\)\)", timeout=timeout)) # Press
        # - Only assert release for the final key
        k = key[-1]
        test_assert(name+" "+k+" release timeout", instance.wait_for_line("\[syscalls\] - USER> (Window|Menu)::handle_event\(ev=KeyUp\("+KEYNAME_MAP[k]+"\)\)", timeout=timeout))
    else:
        keyname = KEYNAME_MAP[key]
        instance.type_key(key)
        test_assert(name+" "+key+" press timeout", instance.wait_for_line("\[syscalls\] - USER> (Window|Menu)::handle_event\(ev=KeyDown\("+keyname+"\)\)", timeout=timeout)) # Press
        test_assert(name+" "+key+" release timeout", instance.wait_for_line("\[syscalls\] - USER> (Window|Menu)::handle_event\(ev=KeyUp\("+keyname+"\)\)", timeout=timeout))
        if idle:
            test_assert(name+" "+key+" release idle", instance.wait_for_idle(timeout=5))
def _mouseto(instance, name, x,y):
    instance.mouse_to(x,y)
    test_assert(name+" mouse(%i,%i) reached" % (x,y,), instance.wait_for_line("windows\] - CursorPos::update - \(%i,%i\)" % (x,y), timeout=3))
    test_assert(name+" mouse(%i,%i) idle" % (x,y,), instance.wait_for_idle(timeout=5))
#def _mouseclick_at(instance, name, x,y btn):
#    _mouseto(instance, name, x,y)
#    instance.mouse_press(btn) 
def _mouseclick(instance, name, btn):
    instance.mouse_press(btn) 
    test_assert("%s mouse(%i down) event" % (name, btn,), instance.wait_for_line("syscalls\] - USER> Window::handle_event\(ev=MouseDown\(\d+, \d+, %i\)\)" % (btn-1,), timeout=3))
    test_assert(name+" mouse(%i down) idle" % (btn,), instance.wait_for_idle(timeout=5))
    time.sleep(0.5)
    instance.mouse_release(btn) 
    test_assert("%s mouse(%i up) event" % (name, btn,), instance.wait_for_line("syscalls\] - USER> Window::handle_event\(ev=MouseUp\(\d+, \d+, %i\)\)" % (btn-1,), timeout=3))
    test_assert(name+" mouse(%i up) idle" % (btn,), instance.wait_for_idle(timeout=5))

def test(instance):
    test_assert("Kernel image start timed out", instance.wait_for_line("OK43e6H", timeout=10))
    test_assert("Init load timed out", instance.wait_for_line("Entering userland at 0x[0-9a-f]+ '/system/Tifflin/bin/loader' '/system/Tifflin/bin/init'", timeout=5))

    test_assert("Initial startup timed out", instance.wait_for_idle(timeout=20))
    instance.screenshot('Login')

    instance.type_string('root')
    while instance.wait_for_idle():
        pass
    _keypress(instance, 'ret', "Username")
    # TODO: Have an item in the log here
    
    instance.type_string('password')
    # - Wait until there's 1s with no action
    while instance.wait_for_idle():
        pass
    _keypress(instance, 'ret', "Password", idle=False)
    test_assert("Shell startup timeout", instance.wait_for_line("\[syscalls\] - USER> Calling entry 0x[0-9a-f]+ for b\"/sysroot/bin/shell\"", timeout=1))
    test_assert("Shell idle timeout", instance.wait_for_idle(timeout=5))
    instance.screenshot('Shell')
    # TODO: Have an item in the log here

    # >>> Spawn the GUI terminal
    if True:
        # - Open the "System" menu (press left windows key)
        _keypress(instance, 'meta_l', "System menu")
        instance.screenshot('Menu')

        # - Select the top item to open the CLI
        _keypress(instance, 'ret', "CLI Startup", idle=False)
        test_assert("CLI startup timeout", instance.wait_for_line("\[syscalls\] - USER> Calling entry 0x[0-9a-f]+ for b\"/sysroot/bin/simple_console\"", timeout=5))
        test_assert("CLI window render", instance.wait_for_line("\[gui::windows\] - L\d+: WindowGroup::redraw: \d+ 'Console'", timeout=5))
        test_assert("CLI idle timeout", instance.wait_for_idle(timeout=3))
        instance.screenshot('CLI')

        # - Run a command
        instance.type_string('ls /system')
        while instance.wait_for_idle():
            pass
        instance.type_string('/Tifflin/bin')
        while instance.wait_for_idle():
            pass
        _keypress(instance, 'ret', "Run `ls`")
        instance.screenshot('ls')

        # - Quit shell
        instance.type_string('exit')
        while instance.wait_for_idle():
            pass
        instance.type_key('ret')
        test_assert("`exit` return release timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyUp\(Return\)\)", timeout=1)) # Release
        test_assert("`exit` reap", instance.wait_for_line("Reaping thread 0x[0-9a-f]+\(\d+ /sysroot/bin/simple_console#1\)", timeout=2))
        instance.screenshot('exit')
        # DISABLED: Idle triggers reaping
        #test_assert("`ls` idle timeout", instance.wait_for_idle(timeout=5))
        
        # - Ensure that the GUI re-renders, and that the terminal no-longer shows
        test_assert("final render", instance.wait_for_line("WindowGroup::redraw: render_order=\[\(1, \[\]\), \(4, \[\(0,20 \+ \d+x\d+\)\]\), \(5, \[\(0,0 \+ \d+x20\)\]\)\]", timeout=5))
        test_assert("Final render idle", instance.wait_for_idle(timeout=5))
    
    # >>> Start the filesystem browser
    if True:
        _keypress(instance, 'meta_l', "Menu 2")
        _keypress(instance, 'down', "Menu 2")
        _keypress(instance, 'down', "Menu 2")
        _keypress(instance, 'ret', "Menu 2", idle=False)
        test_assert("FileBrowser window render", instance.wait_for_line("\[gui::windows\] - L\d+: WindowGroup::redraw: \d+ 'File browser'", timeout=5))
        test_assert("FileBrowser idle timeout", instance.wait_for_idle(timeout=3))
        instance.screenshot('FileBrowser')
        
        _keypress(instance, 'down', "File browser")
        #test_assert("FileBrowser window render", instance.wait_for_idle(timeout=5))
        instance.screenshot('FileBrowser-sel2')
        #_keypress(instance, 'up', "File browser")
        _keypress(instance, 'ret', "File browser", idle=False)
        test_assert("FileBrowser window render", instance.wait_for_line("\[gui::windows\] - L\d+: WindowGroup::redraw: \d+ 'File browser'", timeout=5))
        test_assert("FileBrowser window render", instance.wait_for_idle(timeout=5))
        instance.screenshot('FileBrowser-enter2')
        
        # Close using alt-f4
        instance.type_combo(['alt', 'f4'])
        test_assert("Close FileBrowser alt press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyDown\(LeftAlt\)\)", timeout=1))
        test_assert("Close FileBrowser f4 press timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyDown\(F4\)\)", timeout=1))
        test_assert("Close FileBrowser f4 release timeout", instance.wait_for_line("\[syscalls\] - USER> Window::handle_event\(ev=KeyUp\(F4\)\)", timeout=1))
        test_assert("Close FileBrowser reap", instance.wait_for_line("Reaping thread 0x[0-9a-f]+\(\d+ /sysroot/bin/filebrowser#1\)", timeout=2))
    # >>> Start the filesystem browser (again)
    if False:
        _keypress(instance, ['meta_l', 'e'], "FileBrowser")
        test_assert("FileBrowser window render", instance.wait_for_line("\[gui::windows\] - L\d+: WindowGroup::redraw: \d+ 'File browser'", timeout=5))
        test_assert("FileBrowser idle timeout", instance.wait_for_idle(timeout=3))
        instance.screenshot('FileBrowser2')
        
        # - By default, WTK creates 250x200 windows at (150,100)
        _mouseto(instance, "Exit button", 150 + 250 - 6, 100 + 6)
        instance.screenshot('mouse')
        time.sleep(1)
        _mouseclick(instance, "Exit button", 1)
        test_assert("Close FileBrowser reap", instance.wait_for_line("Reaping thread 0x[0-9a-f]+\(\d+ /sysroot/bin/filebrowser#1\)", timeout=2))
        


    while instance.wait_for_idle():
        pass


try:
    test( TestInstance.Instance("amd64", "CLI") )
except TestInstance.TestFail as e:
    print "TEST FAILURE:",e
    sys.exit(1)