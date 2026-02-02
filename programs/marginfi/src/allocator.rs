/*
================================================================================
MARGINFI CUSTOM BUMP HEAP ALLOCATOR
================================================================================

This module provides a custom memory allocator optimized for Marginfi's risk
engine operations, specifically enabling support for up to 16 positions without
requiring `requestHeapFrame` for typical operations.

================================================================================
COMPARISON: NATIVE SOLANA VS MARGINFI ALLOCATOR
================================================================================

┌─────────────────────┬────────────────────────┬────────────────────────────────┐
│ Feature             │ Native Solana          │ Marginfi                       │
├─────────────────────┼────────────────────────┼────────────────────────────────┤
│ Heap Size           │ 32 KiB                 │ 256 KiB (8 × 32 KiB)           │
│ Direction           │ Backward (top-down)    │ Forward (bottom-up)            │
│ Start Address       │ HEAP_START + HEAP_LEN  │ HEAP_START + sizeof(ptr)       │
│ Position Storage    │ Internal struct        │ At HEAP_START (first 8 bytes)  │
│ Heap Reuse          │ Not supported          │ heap_pos() / heap_restore()    │
│ requestHeapFrame    │ Required for >32 KiB   │ Only for >32 KiB operations    │
└─────────────────────┴────────────────────────┴────────────────────────────────┘

## Native Solana Allocator (solana_program::entrypoint)

The default Solana bump allocator works as follows:

```
HEAP_START_ADDRESS: 0x300000000
HEAP_LENGTH: 32 * 1024 (32 KiB)

Allocation Direction: BACKWARD (High → Low)
─────────────────────────────────────────────

0x300000000           0x300008000
      │                    │
      ▼                    ▼
      [====================]
                          ▲
                          │
                 pos starts here, moves LEFT with each alloc

Code (simplified):
    pos = self.start + self.len;        // Start at top (0x300008000)
    pos = pos.saturating_sub(size);     // Move down
    pos &= !(align - 1);                // Align
```

## Why Backward Allocation Breaks Without requestHeapFrame

The key issue with the default backward allocation is NOT that it's incompatible with extended heaps,
but rather that it REQUIRES `requestHeapFrame` to be called when compiled with a larger
HEAP_LENGTH. Here's the problem:

**Critical insight**: The compiled allocator is completely UNAWARE of what the VM actually
provides. It only knows its compiled HEAP_LENGTH constant. When a program is compiled with
HEAP_LENGTH > 32 KiB (e.g., 256 KiB), EVERY allocation assumes that much space is available.
The allocator calculates its starting position as: `pos = HEAP_START + HEAP_LENGTH`

The Solana VM, however, only makes 32 KiB accessible by DEFAULT. The VM boundary is
only extended when `requestHeapFrame` is explicitly called. Without it, the VM has no
way to know the program was compiled with a larger heap expectation.

This creates a fundamental mismatch:
  - Program thinks it has: HEAP_LENGTH bytes (compiled constant, e.g., 256 KiB)
  - VM actually provides: 32 KiB (default) or requestHeapFrame size

The allocator cannot dynamically adapt to the actual heap size—it blindly uses the
compiled constant. (See: https://github.com/solana-labs/solana/issues/32607)

Practical implication: if you compile the backward allocator with HEAP_LENGTH > 32
KiB (say 128 KiB), every transaction that invokes the program must include a
`request_heap_frame` of at least that size (up to the 256 KiB cap) before the
program call. Otherwise, the very first allocation points outside the VM-mapped
32 KiB and will fault, even if that instruction only needs a few bytes.

If you compile with HEAP_LENGTH = 256 KiB and request only a 64 KiB heap frame,
the backward allocator still starts at +256 KiB and the first allocation will
point outside the mapped 64 KiB, so you must request at least the compiled
HEAP_LENGTH (up to the 256 KiB cap).

```
Without requestHeapFrame (VM provides 32 KiB, program compiled for 256 KiB):
────────────────────────────────────────────────────────────────────────────
0x300000000           0x300008000                          0x300040000
      │                    │                                     │
      ▼                    ▼                                     ▼
      [====================│ · · · · inaccessible · · · · · · · ·]
      ^                    ^                                     ^
      │                    │                                     │
 VM Lower              VM Upper                     Backward allocator starts HERE!
 Boundary              Boundary                     ACCESS VIOLATION!
 (accessible)          (32 KiB limit)               (outside VM boundary)
```

This means backward allocation with extended HEAP_LENGTH:
  ✗ FAILS without requestHeapFrame (starts outside VM boundary)
  ✓ WORKS with requestHeapFrame (VM boundary extended to match)

## Marginfi Solution: Forward Allocation

By allocating FORWARD from the bottom, we achieve universal compatibility. The key insight
is that the starting position (HEAP_START + sizeof(ptr)) is ALWAYS within the VM-accessible
region, regardless of what the VM provides. The forward allocator grows toward whatever
boundary exists—be it 32 KiB (default) or 256 KiB (with requestHeapFrame):

```
Scenario A - Without requestHeapFrame (default 32 KiB VM boundary):
────────────────────────────────────────────────────────────────────────────
0x300000000           0x300008000                          0x300040000
      │                    │                                     │
      ▼                    ▼                                     ▼
      [====================│ · · · · inaccessible · · · · · · · ·]
      ▲        ──▶        ▲
      │                   │
 pos starts HERE     Allocations stay within 32 KiB
 (ALWAYS SAFE!)      (operations that fit in 32 KiB work fine)


Scenario B - With requestHeapFrame (extended 256 KiB VM boundary):
────────────────────────────────────────────────────────────────────────────
0x300000000           0x300008000                          0x300040000
      │                    │                                     │
      ▼                    ▼                                     ▼
      [====================│=====================================]
      ▲                                         ──▶              ▲
      │                                                          │
 pos starts HERE                                        Full 256 KiB available
 (SAME starting point!)                                 (for large operations)

Code (this allocator):
    pos = HEAP_START + sizeof(ptr);     // Start at bottom (after reserved)
    pos = align_up(pos, align);         // Align upward
    pos = pos + size;                   // Move up
```

The forward allocator is compatible with BOTH scenarios:
  ✓ Functions NOT needing extra heap: work without requestHeapFrame (stay within 32 KiB)
  ✓ Functions needing extra heap: work with requestHeapFrame (can use up to 256 KiB)

This flexibility allows the same compiled program to handle both small operations
(within default heap) and large operations (with extended heap) without code changes.
Note: when compiled for 256 KiB, this allocator only bounds to that compiled
limit. If you allocate past ~32 KiB without calling `requestHeapFrame`, the
allocator will still hand back a pointer, but the VM will fault once you touch
the unmapped region. You must still request a larger heap before using more than
the default 32 KiB.

**What happens when allocations exceed the VM boundary?**
  - Backward allocator: FAILS IMMEDIATELY at first allocation (starts outside boundary)
  - Forward allocator: Works fine up to the VM-mapped size (32 KiB by default).
    Beyond that, accesses will fault unless `requestHeapFrame` expanded the heap.
    The `null_mut()` guard only triggers past the compiled 256 KiB ceiling, so
    always request a heap frame before relying on >32 KiB allocations.

================================================================================
KEY IMPROVEMENTS OVER NATIVE SOLANA ALLOCATOR
================================================================================

1. GRACEFUL DEGRADATION
   - Operations using ≤32 KiB work without requestHeapFrame
   - Operations needing >32 KiB automatically work when requestHeapFrame is used
   - No code changes needed between the two modes

2. HEAP POSITION TRACKING
   - Position pointer stored at HEAP_START_ADDRESS (first 8 bytes)
   - Enables runtime heap introspection via heap_pos()
   - Enables temporary allocation patterns via heap_restore()

3. MEMORY RECYCLING (Critical for Risk Engine)
   - Native Solana: No way to reclaim memory mid-execution
   - Marginfi: heap_restore() allows "freeing" temporary allocations

   Measured heap usage per position (oracle adapter only):
     - Pyth Push oracles: ~64 bytes
     - Switchboard Pull oracles: ~128 bytes

   Without recycling, all positions stay allocated simultaneously:
   ┌────────┬────────┬────────┬─────────────────┬────────┐
   │ Pos 1  │ Pos 2  │ Pos 3  │ ... all stay ...│ Pos 16 │
   └────────┴────────┴────────┴─────────────────┴────────┘

   With recycling (same memory reused, peak ~128 bytes):
   ┌────────┐     ┌────────┐           ┌────────┐
   │ Pos 1  │ ──▶ │ Pos 2  │ ──▶ ... ─▶│ Pos 16 │  Minimal footprint!
   └────────┘     └────────┘           └────────┘
       ▼              ▼                     ▼
    restore()     restore()            restore()

================================================================================
SAFETY REQUIREMENTS FOR heap_restore()
================================================================================

⚠️  CRITICAL: heap_restore() does NOT run destructors. It simply resets the
    allocation pointer, making the memory available for reuse.

✓ SAFE patterns:
  - Extract primitive values (I80F48, u64, bool, [u8; N]) to stack before restore
  - Use Copy types that don't need heap cleanup
  - Drop all Boxes/Vecs/Strings naturally before restore

✗ UNSAFE patterns:
  - Holding references to heap data across restore boundary
  - Accessing Box/Vec/String contents after restore
  - Storing heap pointers in struct fields that survive restore

Example of SAFE usage:
```rust
let checkpoint = heap_pos();
{
    let temp_vec: Vec<u8> = vec![1, 2, 3];          // Heap allocation
    let sum: u64 = temp_vec.iter().sum();           // Result stored on stack
    // temp_vec goes out of scope here (but dealloc is no-op anyway)
}
heap_restore(checkpoint);
// sum is valid (stack), temp_vec's memory is now available for reuse
```

================================================================================
REFERENCES
================================================================================

Native Solana allocator source:
  - https://github.com/solana-labs/solana/blob/master/sdk/program/src/entrypoint.rs

Squads Forward heap allocator implementation:
  - https://github.com/Squads-Grid/external-signature-program/blob/main/src/allocator.rs

*/

#[cfg(target_os = "solana")]
use anchor_lang::solana_program::entrypoint::HEAP_START_ADDRESS;
#[cfg(target_os = "solana")]
use std::{alloc::Layout, mem::size_of, ptr::null_mut};
#[cfg(target_os = "solana")]
pub const HEAP_LENGTH: usize = 8 * 32 * 1024;
#[cfg(target_os = "solana")]
pub struct BumpAllocator;

#[cfg(target_os = "solana")]
unsafe impl std::alloc::GlobalAlloc for BumpAllocator {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        const POS_PTR: *mut usize = HEAP_START_ADDRESS as *mut usize;
        const TOP_ADDRESS: usize = HEAP_START_ADDRESS as usize + HEAP_LENGTH;
        const BOTTOM_ADDRESS: usize = HEAP_START_ADDRESS as usize + size_of::<*mut u8>();

        let mut pos = *POS_PTR;
        if pos == 0 {
            // First time, set starting position to bottom address
            pos = BOTTOM_ADDRESS;
        }

        pos = (pos + layout.align() - 1) & !(layout.align() - 1);

        let next_pos = pos.saturating_add(layout.size());

        // Check if we've exceeded the heap limit
        // Note: This is our configured limit (256 KiB), but the VM boundary
        // may be smaller (32 KiB default) without requestHeapFrame
        if next_pos > TOP_ADDRESS {
            return null_mut();
        }

        // Update position pointer and return allocated address
        *POS_PTR = next_pos;
        pos as *mut u8
    }

    #[inline]
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}

#[cfg(all(
    target_os = "solana",
    feature = "custom-heap",
    not(feature = "no-entrypoint")
))]
#[global_allocator]
static ALLOCATOR: BumpAllocator = BumpAllocator;

/// Returns the current heap position for creating a checkpoint.
#[cfg(all(target_os = "solana", feature = "custom-heap"))]
#[inline]
pub fn heap_pos() -> usize {
    unsafe {
        let pos_ptr = HEAP_START_ADDRESS as *mut usize;
        let pos = *pos_ptr;
        if pos == 0 {
            // Heap hasn't been used yet, return bottom address
            HEAP_START_ADDRESS as usize + size_of::<*mut u8>()
        } else {
            pos
        }
    }
}

/// Restores the heap position to a previously saved checkpoint.
#[cfg(all(target_os = "solana", feature = "custom-heap"))]
#[inline]
pub fn heap_restore(pos: usize) {
    unsafe {
        let pos_ptr = HEAP_START_ADDRESS as *mut usize;
        *pos_ptr = pos;
    }
}

#[cfg(not(all(target_os = "solana", feature = "custom-heap")))]
#[inline]
pub fn heap_pos() -> usize {
    0
}

#[cfg(not(all(target_os = "solana", feature = "custom-heap")))]
#[inline]
pub fn heap_restore(_pos: usize) {}
