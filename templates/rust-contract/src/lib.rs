#![no_std]

extern crate alloc;

use core::alloc::{GlobalAlloc, Layout};
use core::panic::PanicInfo;
use truthlinked_sdk::prelude::*;

struct BumpAllocator;

const HEAP_SIZE: usize = 1024 * 256;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
static mut NEXT: usize = 0;

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let align = layout.align().max(1);
        let size = layout.size();
        let start = (NEXT + align - 1) & !(align - 1);
        let end = match start.checked_add(size) {
            Some(v) => v,
            None => return core::ptr::null_mut(),
        };
        if end > HEAP_SIZE {
            core::ptr::null_mut()
        } else {
            NEXT = end;
            core::ptr::addr_of_mut!(HEAP).cast::<u8>().add(start)
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Bump allocator does not reclaim memory during a single execution.
    }
}

#[global_allocator]
static ALLOC: BumpAllocator = BumpAllocator;

#[error_code(base = 11)]
#[derive(Clone, Copy)]
enum ContractError {
    UnknownSelector,
    ZeroAmount,
}

#[derive(Event)]
#[event(name = "CounterUpdated")]
struct CounterUpdated {
    #[topic]
    caller: [u8; 32],
    value: u64,
    action: u8,
}

#[derive(Manifest)]
#[manifest(
    read_derived(namespace = "template.counter", key = "value"),
    write_derived(namespace = "template.counter", key = "value"),
    key_spec(offset = 4, len = 32)
)]
#[allow(dead_code)]
struct TemplateManifest;

fn counter_slot() -> Slot {
    let ns = storage::namespace("template.counter");
    storage::slot_for(&ns, b"value")
}

fn visits_map() -> StorageMap<u64> {
    let ns = storage::namespace("template.counter.visits");
    StorageMap::new(Namespace::new(ns))
}

fn history_vec() -> StorageVec<u64> {
    let ns = storage::namespace("template.counter.history");
    StorageVec::new(Namespace::new(ns))
}

fn emit_counter_event(caller: [u8; 32], value: u64, action: u8) -> Result<()> {
    log::emit_event(&CounterUpdated {
        caller,
        value,
        action,
    })
}

#[require(amount > 0, ContractError::ZeroAmount)]
fn ensure_positive(amount: u64) -> Result<()> {
    let _ = amount;
    Ok(())
}

fn handle_execute() -> Result<()> {
    let calldata = context::calldata()?;
    let selector = abi::selector(&calldata)?;

    let sel_increment = abi::selector_of("increment");
    let sel_set = abi::selector_of("set");
    let sel_get = abi::selector_of("get");
    let sel_get_visits = abi::selector_of("get_visits");
    let sel_history_len = abi::selector_of("history_len");

    if selector == sel_increment {
        let amount = if calldata.len() >= 12 {
            abi::read_u64(&calldata, 4)?
        } else {
            1
        };
        ensure_positive(amount)?;
        let counter = counter_slot();
        let next = counter.read_u64()?.saturating_add(amount);
        counter.write_u64(next)?;

        let caller = context::caller()?;
        let visits = visits_map().get_typed_key(&caller)?.unwrap_or(0);
        visits_map().insert_typed_key(&caller, &visits.saturating_add(1))?;

        let _ = history_vec().push(&next)?;
        emit_counter_event(caller, next, 1)?;
        return Ok(());
    }

    if selector == sel_set {
        let next = abi::read_u64(&calldata, 4)?;
        let counter = counter_slot();
        counter.write_u64(next)?;
        let _ = history_vec().push(&next)?;
        emit_counter_event(context::caller()?, next, 2)?;
        return Ok(());
    }

    if selector == sel_get {
        let counter = counter_slot();
        let value = counter.read_u64()?;
        context::set_return_data(&value.to_le_bytes())?;
        return Ok(());
    }

    if selector == sel_get_visits {
        let account = if calldata.len() >= 36 {
            abi::read_account(&calldata, 4)?
        } else {
            context::caller()?
        };
        let visits = visits_map().get_typed_key(&account)?.unwrap_or(0);
        context::set_return_data(&visits.to_le_bytes())?;
        return Ok(());
    }

    if selector == sel_history_len {
        let len = history_vec().len()?;
        context::set_return_data(&len.to_le_bytes())?;
        return Ok(());
    }

    Err(ContractError::UnknownSelector.into())
}

contract_entry!(handle_execute);

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
