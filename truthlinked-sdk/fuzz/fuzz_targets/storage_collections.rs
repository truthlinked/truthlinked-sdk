//! Fuzz target for storage collections (StorageMap and StorageVec).
//!
//! This fuzzer tests the correctness of storage collections by comparing
//! their behavior against standard library equivalents (BTreeMap and Vec).
//!
//! It generates random sequences of operations and verifies that:
//! - Map operations match BTreeMap behavior
//! - Vector operations match Vec behavior
//! - Storage state remains consistent

#![no_main]

use arbitrary::{Arbitrary, Unstructured};
use libfuzzer_sys::fuzz_target;
use truthlinked_sdk::prelude::*;

const NS_MAP: Namespace = Namespace([0x11; 32]);
const NS_VEC: Namespace = Namespace([0x22; 32]);

/// Operations that can be performed on storage collections.
#[derive(Clone, Debug)]
enum Op {
    MapInsert { key: [u8; 32], value: u64 },
    MapRemove { key: [u8; 32] },
    MapGet { key: [u8; 32] },
    VecPush { value: u64 },
    VecPop,
    VecSet { index: u16, value: u64 },
    VecGet { index: u16 },
    VecClear,
}

impl<'a> Arbitrary<'a> for Op {
    fn arbitrary(u: &mut Unstructured<'a>) -> arbitrary::Result<Self> {
        let tag = u.int_in_range::<u8>(0..=7)?;
        match tag {
            0 => Ok(Op::MapInsert {
                key: u.arbitrary()?,
                value: u.arbitrary()?,
            }),
            1 => Ok(Op::MapRemove { key: u.arbitrary()? }),
            2 => Ok(Op::MapGet { key: u.arbitrary()? }),
            3 => Ok(Op::VecPush {
                value: u.arbitrary()?,
            }),
            4 => Ok(Op::VecPop),
            5 => Ok(Op::VecSet {
                index: u.arbitrary()?,
                value: u.arbitrary()?,
            }),
            6 => Ok(Op::VecGet {
                index: u.arbitrary()?,
            }),
            _ => Ok(Op::VecClear),
        }
    }
}

#[derive(Clone, Debug, Arbitrary)]
struct Input {
    ops: Vec<Op>,
}

fuzz_target!(|input: Input| {
    let mut harness = StorageHarness::new();
    let mut map_model = std::collections::BTreeMap::<[u8; 32], u64>::new();
    let mut vec_model = Vec::<u64>::new();

    for op in input.ops.into_iter().take(1024) {
        match op {
            Op::MapInsert { key, value } => {
                {
                    let mut map = harness.map::<u64>(NS_MAP);
                    let _ = map.insert(&key, &value);
                }
                map_model.insert(key, value);
            }
            Op::MapRemove { key } => {
                {
                    let mut map = harness.map::<u64>(NS_MAP);
                    let _ = map.remove(&key);
                }
                map_model.remove(&key);
            }
            Op::MapGet { key } => {
                let got = {
                    let map = harness.map::<u64>(NS_MAP);
                    map.get(&key).unwrap_or(None)
                };
                let expected = map_model.get(&key).copied();
                assert_eq!(got, expected);
            }
            Op::VecPush { value } => {
                {
                    let mut list = harness.vec::<u64>(NS_VEC);
                    let _ = list.push(&value);
                }
                vec_model.push(value);
            }
            Op::VecPop => {
                let got = {
                    let mut list = harness.vec::<u64>(NS_VEC);
                    list.pop().unwrap_or(None)
                };
                let expected = vec_model.pop();
                assert_eq!(got, expected);
            }
            Op::VecSet { index, value } => {
                let idx = (index as usize) % 256;
                let got = {
                    let mut list = harness.vec::<u64>(NS_VEC);
                    list.set(idx as u64, &value).unwrap_or(false)
                };
                if idx < vec_model.len() {
                    vec_model[idx] = value;
                    assert!(got);
                } else {
                    assert!(!got);
                }
            }
            Op::VecGet { index } => {
                let idx = (index as usize) % 256;
                let got = {
                    let list = harness.vec::<u64>(NS_VEC);
                    list.get(idx as u64).unwrap_or(None)
                };
                let expected = vec_model.get(idx).copied();
                assert_eq!(got, expected);
            }
            Op::VecClear => {
                {
                    let mut list = harness.vec::<u64>(NS_VEC);
                    let _ = list.clear();
                }
                vec_model.clear();
            }
        }
    }

    let onchain_len = {
        let list = harness.vec::<u64>(NS_VEC);
        list.len().unwrap_or(0)
    };
    assert_eq!(onchain_len as usize, vec_model.len());
});
