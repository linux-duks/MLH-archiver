from mlh_parser.parser_algorithm import extract_attributions


input_syzbot_mail = """
syzbot has found a reproducer for the following crash on:

HEAD commit:    79c3ba32 Merge tag 'drm-fixes-2019-06-07-1' of git://anong..
git tree:       upstream
console output: https://syzkaller.appspot.com/x/log.txt?x=1201b971a00000
kernel config:  https://syzkaller.appspot.com/x/.config?x=60564cb52ab29d5b
dashboard link: https://syzkaller.appspot.com/bug?extid=2ff1e7cb738fd3c41113
compiler:       gcc (GCC) 9.0.0 20181231 (experimental)
syz repro:      https://syzkaller.appspot.com/x/repro.syz?x=14a3bf51a00000
C reproducer:   https://syzkaller.appspot.com/x/repro.c?x=120d19f2a00000

The bug was bisected to:

commit 0fff724a33917ac581b5825375d0b57affedee76
Author: Paul Kocialkowski <paul.kocialkowski@bootlin.com>
Date:   Fri Jan 18 14:51:13 2019 +0000

     drm/sun4i: backend: Use explicit fourcc helpers for packed YUV422 check

bisection log:  https://syzkaller.appspot.com/x/bisect.txt?x=1467550f200000
final crash:    https://syzkaller.appspot.com/x/report.txt?x=1667550f200000
console output: https://syzkaller.appspot.com/x/log.txt?x=1267550f200000

IMPORTANT: if you fix the bug, please add the following tag to the commit:
Reported-by: syzbot+2ff1e7cb738fd3c41113@syzkaller.appspotmail.com
Fixes: 0fff724a3391 ("drm/sun4i: backend: Use explicit fourcc helpers for  
packed YUV422 check")

WARNING: CPU: 0 PID: 8951 at kernel/bpf/core.c:851 bpf_jit_free+0x157/0x1b0
Kernel panic - not syncing: panic_on_warn set ...
CPU: 0 PID: 8951 Comm: kworker/0:0 Not tainted 5.2.0-rc3+ #23
Hardware name: Google Google Compute Engine/Google Compute Engine, BIOS  
Google 01/01/2011
Workqueue: events bpf_prog_free_deferred
Call Trace:
  __dump_stack lib/dump_stack.c:77 [inline]
  dump_stack+0x172/0x1f0 lib/dump_stack.c:113
  panic+0x2cb/0x744 kernel/panic.c:219
  __warn.cold+0x20/0x4d kernel/panic.c:576
  report_bug+0x263/0x2b0 lib/bug.c:186
  fixup_bug arch/x86/kernel/traps.c:179 [inline]
  fixup_bug arch/x86/kernel/traps.c:174 [inline]
  do_error_trap+0x11b/0x200 arch/x86/kernel/traps.c:272
  do_invalid_op+0x37/0x50 arch/x86/kernel/traps.c:291
  invalid_op+0x14/0x20 arch/x86/entry/entry_64.S:986
RIP: 0010:bpf_jit_free+0x157/0x1b0
Code: 00 fc ff df 48 89 fa 48 c1 ea 03 80 3c 02 00 75 5d 48 b8 00 02 00 00  
00 00 ad de 48 39 43 70 0f 84 05 ff ff ff e8 f9 b5 f4 ff <0f> 0b e9 f9 fe  

"""


def test_syzbot_email() -> None:
    attr = extract_attributions(input_syzbot_mail)
    assert len(attr) == 0


input_example_mail = """
Email blabla
Signed-off-by: Example Contributor <example@contributor.com>
"""


def test_correct_email() -> None:
    attr = extract_attributions(input_example_mail)
    assert len(attr) == 1
    assert attr == [
        {
            "attribution": "Signed-off-by",
            "identification": "Example Contributor <example@contributor.com>",
        }
    ]
