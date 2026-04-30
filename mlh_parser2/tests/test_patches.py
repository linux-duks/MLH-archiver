from mlh_parser.parser_algorithm import extract_patches

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
    output = extract_patches(input_syzbot_mail)
    assert len(output) == 0


input_example_mail = """

Interfaces for when a new domain in the crashdump kernel needs some
values from the panicked kernel's context entries.

Signed-off-by: Bill Sumner 
---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
 
 
 #ifdef CONFIG_CRASH_DUMP
+
 /*
  * Fix Crashdump failure caused by leftover DMA through a hardware IOMMU
  *
@@ -437,6 +438,16 @@ static int intel_iommu_copy_translation_tables(struct dmar_drhd_unit *drhd,
                struct root_entry **root_new_virt_p,
                int g_num_of_iommus);
 
+static struct domain_values_entry *intel_iommu_did_to_domain_values_entry(
+               int did, struct intel_iommu *iommu);
+
+static int intel_iommu_get_dids_from_old_kernel(struct intel_iommu *iommu);
+
+static struct domain_values_entry *intel_iommu_did_to_domain_values_entry(
+               int did, struct intel_iommu *iommu);
+
+static int intel_iommu_get_dids_from_old_kernel(struct intel_iommu *iommu);
+
 #endif /* CONFIG_CRASH_DUMP */
 
 /*
@@ -5230,4 +5241,39 @@ static int intel_iommu_copy_translation_tables(struct dmar_drhd_unit *drhd,
        return 0;
 }
 
+/*
+ * Interfaces for when a new domain in the crashdump kernel needs some
+ * values from the panicked kernel's context entries
+ *
+ */
+static struct domain_values_entry *intel_iommu_did_to_domain_values_entry(
+               int did, struct intel_iommu *iommu)
+{
+       struct domain_values_entry *dve;        /* iterator */
+
+       list_for_each_entry(dve, &domain_values_list[iommu->seq_id], link)
+               if (dve->did == did)
+                       return dve;
+       return NULL;
+}
+
+/* Mark domain-id's from old kernel as in-use on this iommu so that a new
+ * domain-id is allocated in the case where there is a device in the new kernel
+ * that was not in the old kernel -- and therefore a new domain-id is needed.
+ */
+static int intel_iommu_get_dids_from_old_kernel(struct intel_iommu *iommu)
+{
+       struct domain_values_entry *dve;        /* iterator */
+
+       pr_info("IOMMU:%d Domain ids from panicked kernel:\n", iommu->seq_id);
+
+       list_for_each_entry(dve, &domain_values_list[iommu->seq_id], link) {
+               set_bit(dve->did, iommu->domain_ids);
+               pr_info("DID did:%d(0x%4.4x)\n", dve->did, dve->did);
+       }
+
+       pr_info("----------------------------------------\n");
+       return 0;
+}
+
 #endif /* CONFIG_CRASH_DUMP */
-- 
2.0.0-rc0

""".strip()


diff = [
    """---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
 
 
 #ifdef CONFIG_CRASH_DUMP
+
 /*
  * Fix Crashdump failure caused by leftover DMA through a hardware IOMMU
  *
@@ -437,6 +438,16 @@ static int intel_iommu_copy_translation_tables(struct dmar_drhd_unit *drhd,
                struct root_entry **root_new_virt_p,
                int g_num_of_iommus);
 
+static struct domain_values_entry *intel_iommu_did_to_domain_values_entry(
+               int did, struct intel_iommu *iommu);
+
+static int intel_iommu_get_dids_from_old_kernel(struct intel_iommu *iommu);
+
+static struct domain_values_entry *intel_iommu_did_to_domain_values_entry(
+               int did, struct intel_iommu *iommu);
+
+static int intel_iommu_get_dids_from_old_kernel(struct intel_iommu *iommu);
+
 #endif /* CONFIG_CRASH_DUMP */
 
 /*
@@ -5230,4 +5241,39 @@ static int intel_iommu_copy_translation_tables(struct dmar_drhd_unit *drhd,
        return 0;
 }
 
+/*
+ * Interfaces for when a new domain in the crashdump kernel needs some
+ * values from the panicked kernel's context entries
+ *
+ */
+static struct domain_values_entry *intel_iommu_did_to_domain_values_entry(
+               int did, struct intel_iommu *iommu)
+{
+       struct domain_values_entry *dve;        /* iterator */
+
+       list_for_each_entry(dve, &domain_values_list[iommu->seq_id], link)
+               if (dve->did == did)
+                       return dve;
+       return NULL;
+}
+
+/* Mark domain-id's from old kernel as in-use on this iommu so that a new
+ * domain-id is allocated in the case where there is a device in the new kernel
+ * that was not in the old kernel -- and therefore a new domain-id is needed.
+ */
+static int intel_iommu_get_dids_from_old_kernel(struct intel_iommu *iommu)
+{
+       struct domain_values_entry *dve;        /* iterator */
+
+       pr_info("IOMMU:%d Domain ids from panicked kernel:\n", iommu->seq_id);
+
+       list_for_each_entry(dve, &domain_values_list[iommu->seq_id], link) {
+               set_bit(dve->did, iommu->domain_ids);
+               pr_info("DID did:%d(0x%4.4x)\n", dve->did, dve->did);
+       }
+
+       pr_info("----------------------------------------\n");
+       return 0;
+}
+
 #endif /* CONFIG_CRASH_DUMP */
-- 
2.0.0-rc0
""".strip(),
]


def test_correct_email() -> None:
    output = extract_patches(input_example_mail)
    assert output == diff


# --- Test Input Definitions ---

# Test Case 1: No patch content
input_syzbot_mail = """
syzbot has found a reproducer for the following crash on:

HEAD commit:    79c3ba32 Merge tag 'drm-fixes-2019-06-07-1' of git://anong..
git tree:       upstream
console output: https://syzkaller.appspot.com/x/log.txt?x=1201b971a00000
kernel config:  https://syzkaller.appspot.com/x/.config?x=60564cb52ab29d5b
dashboard link: https://syzkaller.appspot.com/bug?extid=2ff1e7cb738fd3c41113
compiler:       gcc (GCC) 9.0.0 20181231 (experimental)
syz repro:      https://syzkaller.appspot.com/x/repro.syz?x=14a3bf51a00000
C reproducer:   https://syzkaller.appspot.com/x/repro.c?x=120d19f2a00000
"""


input_single_patch_mail = """
Interfaces for when a new domain in the crashdump kernel needs some
values from the panicked kernel's context entries.

Signed-off-by: Bill Sumner 
---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
#endif /* CONFIG_CRASH_DUMP */
--
2.0.0-rc0

""".strip()

expected_single_patch = [
    """---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
#endif /* CONFIG_CRASH_DUMP */
--
2.0.0-rc0
""".strip(),
]

input_multiple_patches_mail = """
This is the first patch in the series. It fixes a minor typo.
Signed-off-by: Joe Developer <joe@example.com>
---
 drivers/net/ethernet/intel/e1000/e1000_main.c | 1 +
 1 file changed, 1 insertion(+)

diff --git a/drivers/net/ethernet/intel/e1000/e1000_main.c b/drivers/net/ethernet/intel/e1000/e1000_main.c
index a2d9b23..b3f1c40 100644
--- a/drivers/net/ethernet/intel/e1000/e1000_main.c
+++ b/drivers/net/ethernet/intel/e1000/e1000_main.c
@@ -512,6 +512,7 @@ static void e1000_reset_task(struct work_struct *work)
     /* Fix the typo in the previous comment */
+     /* New line of code */
     e1000_reset(adapter);
 }
-- 
2.30.0


This is the second patch. It adds a function.
Signed-off-by: Jane Developer <jane@example.com>
---
 include/linux/random.h | 11 +++++++++++
 1 file changed, 11 insertions(+)

diff --git a/include/linux/random.h b/include/linux/random.h
index e2c9a1d..f8e3f4c 100644
--- a/include/linux/random.h
+++ b/include/linux/random.h
@@ -5,6 +5,17 @@
 #include <linux/types.h>
 
+/**
+ * pr_get_random_int - Get a cryptographically secure random integer
+ *
+ * Returns a 32-bit random integer.
+ */
+static inline u32 pr_get_random_int(void)
+{
+    return get_random_u32();
+}
+
 extern void get_random_bytes(void *buf, int nbytes);
 extern void get_random_bytes_arch(void *buf, int nbytes);
-- 
2.30.0

""".strip()

expected_multiple_patches = [
    # patch 1
    """---
 drivers/net/ethernet/intel/e1000/e1000_main.c | 1 +
 1 file changed, 1 insertion(+)

diff --git a/drivers/net/ethernet/intel/e1000/e1000_main.c b/drivers/net/ethernet/intel/e1000/e1000_main.c
index a2d9b23..b3f1c40 100644
--- a/drivers/net/ethernet/intel/e1000/e1000_main.c
+++ b/drivers/net/ethernet/intel/e1000/e1000_main.c
@@ -512,6 +512,7 @@ static void e1000_reset_task(struct work_struct *work)
     /* Fix the typo in the previous comment */
+     /* New line of code */
     e1000_reset(adapter);
 }
-- 
2.30.0
""".strip(),
    # patch 2
    """---
 include/linux/random.h | 11 +++++++++++
 1 file changed, 11 insertions(+)

diff --git a/include/linux/random.h b/include/linux/random.h
index e2c9a1d..f8e3f4c 100644
--- a/include/linux/random.h
+++ b/include/linux/random.h
@@ -5,6 +5,17 @@
 #include <linux/types.h>
 
+/**
+ * pr_get_random_int - Get a cryptographically secure random integer
+ *
+ * Returns a 32-bit random integer.
+ */
+static inline u32 pr_get_random_int(void)
+{
+    return get_random_u32();
+}
+
 extern void get_random_bytes(void *buf, int nbytes);
 extern void get_random_bytes_arch(void *buf, int nbytes);
-- 
2.30.0
""".strip(),
]


input_single_patch_mail_without_space_before_git_version = """
Interfaces for when a new domain in the crashdump kernel needs some
values from the panicked kernel's context entries.

Signed-off-by: Bill Sumner 
---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
#endif /* CONFIG_CRASH_DUMP */
--
2.0.0-rc0

""".strip()


expected_patch_mail_without_space_before_git_version = [
    """
---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
#endif /* CONFIG_CRASH_DUMP */
--
2.0.0-rc0
""".strip()
]


input_single_patch_mail_with_many_spaces_before_git_version = """
Interfaces for when a new domain in the crashdump kernel needs some
values from the panicked kernel's context entries.

Signed-off-by: Bill Sumner 
---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
#endif /* CONFIG_CRASH_DUMP */
--              
2.0.0-rc0

""".strip()


expected_patch_mail_with_many_spaces_before_git_version = [
    """
---
 drivers/iommu/intel-iommu.c | 46 +++++++++++++++++++++++++++++++++++++++++++++
 1 file changed, 46 insertions(+)

diff --git a/drivers/iommu/intel-iommu.c b/drivers/iommu/intel-iommu.c
index 73afed4..bde8f22 100644
--- a/drivers/iommu/intel-iommu.c
+++ b/drivers/iommu/intel-iommu.c
@@ -348,6 +348,7 @@ static inline int first_pte_in_page(struct dma_pte *pte)
#endif /* CONFIG_CRASH_DUMP */
--              
2.0.0-rc0
""".strip()
]


# --- Test Functions ---


def test_no_patch_in_email() -> None:
    """Tests an email that clearly contains no patches."""
    output = extract_patches(input_syzbot_mail)
    assert len(output) == 0
    assert output == []


def test_single_patch_full_format() -> None:
    """Tests a single patch email with diffstat and the final separator."""
    output = extract_patches(input_single_patch_mail)
    assert len(output) == 1

    assert output == expected_single_patch


def test_multiple_patches_in_email() -> None:
    """Tests an email containing two patches separated by regular text."""
    output = extract_patches(input_multiple_patches_mail)
    assert len(output) == 2

    # Patch 1 check
    assert output == expected_multiple_patches


def test_mail_without_space_before_git_version() -> None:
    """Tests a single patch email with diffstat and the final separator."""
    output = extract_patches(input_single_patch_mail_without_space_before_git_version)
    assert len(output) == 1

    assert output == expected_patch_mail_without_space_before_git_version


def test_mail_with_many_spaces_before_git_version() -> None:
    """Tests a single patch email with diffstat and the final separator."""
    output = extract_patches(
        input_single_patch_mail_with_many_spaces_before_git_version
    )
    assert len(output) == 1
    expected_diff = expected_patch_mail_with_many_spaces_before_git_version

    assert output == expected_diff
