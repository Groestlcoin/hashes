//! An implementation of the SHA-1 cryptographic hash algorithm.

//! To use this module, first create a `Sha1` object using the `Sha1` constructor,
//! then feed it an input message using the `input` or `input_str` methods,
//! which may be called any number of times; they will buffer the input until
//! there is enough to call the block algorithm.
//!
//! After the entire input has been fed to the hash read the result using
//! the `result` or `result_str` methods. The first will return bytes, and
//! the second will return a `String` object of the same bytes represented
//! in hexadecimal form.
//!
//! The `Sha1` object may be reused to create multiple hashes by calling
//! the `reset()` method. These traits are implemented by all hash digest
//! algorithms that implement the `Digest` trait. An example of use is:
//!
//! ```rust
//! use sha_1::{Sha1, Digest};
//!
//! // create a Sha1 object
//! let mut sh = Sha1::default();
//!
//! // write input message
//! sh.input(b"hello world");
//!
//! // read hash digest in the form of GenericArray which is in this case
//! // equivalent to [u8; 20]
//! let output = sh.result();
//! assert_eq!(output[..], [0x2a, 0xae, 0x6c, 0x35, 0xc9, 0x4f, 0xcf, 0xb4, 0x15, 0xdb,
//!                         0xe9, 0x5f, 0x40, 0x8b, 0x9c, 0xe9, 0x1e, 0xe8, 0x46, 0xed]);
//! ```
//!
//! # Mathematics
//!
//! The mathematics of the SHA-1 algorithm are quite interesting. In its
//! definition, The SHA-1 algorithm uses:
//!
//! * 1 binary operation on bit-arrays:
//!   * "exclusive or" (XOR)
//! * 2 binary operations on integers:
//!   * "addition" (ADD)
//!   * "rotate left" (ROL)
//! * 3 ternary operations on bit-arrays:
//!   * "choose" (CH)
//!   * "parity" (PAR)
//!   * "majority" (MAJ)
//!
//! Some of these functions are commonly found in all hash digest
//! algorithms, but some, like "parity" is only found in SHA-1.

#![no_std]
 #![feature(repr_simd)]
 #![feature(asm)]

extern crate block_buffer;
extern crate byte_tools;
extern crate digest;
extern crate generic_array;


#[cfg(feature = "asm")]
extern crate sha1_asm as utils;
#[cfg(not(feature = "asm"))]
mod utils;

use utils::{compress, u32x4};

use byte_tools::write_u32_be;
use block_buffer::BlockBuffer512;

pub use digest::Digest;
use generic_array::GenericArray;
use generic_array::typenum::{U20, U64};

mod consts;
use consts::{STATE_LEN, H};



/// Structure representing the state of a SHA-1 computation
#[derive(Copy, Clone)]
pub struct Sha1 {
    abcd: u32x4,
    e: u32x4,
    len: u64,
    buffer: BlockBuffer512,
}

impl Default for Sha1 {
    fn default() -> Self {
        Sha1{
            abcd: u32x4(H[0], H[1], H[2], H[3]), e: u32x4(H[4], 0, 0, 0),
            len: 0u64, buffer: Default::default(),
        }
    }
}

impl digest::BlockInput for Sha1 {
    type BlockSize = U64;
}

impl digest::Input for Sha1 {
    #[inline]
    fn process(&mut self, input: &[u8]) {
        // Assumes that `length_bits<<3` will not overflow
        self.len += input.len() as u64;
        let abcd = &mut self.abcd;
        let e = &mut self.e;
        self.buffer.input(input, |d| compress(abcd, e, d));
    }
}

impl digest::FixedOutput for Sha1 {
    type OutputSize = U20;

    #[inline]
    fn fixed_result(mut self) -> GenericArray<u8, Self::OutputSize> {
        let mut out = GenericArray::default();

        {
            let abcd = &mut self.abcd;
            let e = &mut self.e;
            let len_bits = self.len << 3;
            self.buffer.len_padding(len_bits.to_be(), |d| compress(abcd, e, d));
        }

        write_u32_be(&mut out[..4], self.abcd.0);
        write_u32_be(&mut out[4..8], self.abcd.1);
        write_u32_be(&mut out[8..12], self.abcd.2);
        write_u32_be(&mut out[12..16], self.abcd.3);
        write_u32_be(&mut out[16..20], self.e.0);
        out
    }
}
