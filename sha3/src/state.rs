use keccak;
use byte_tools::{read_u64v_le, write_u64v_le};

const PLEN: usize = 25;

#[derive(Clone, Default)]
pub(crate) struct Sha3State {
    pub state: [u64; PLEN],
}

impl Sha3State {
    #[inline(always)]
    pub(crate) fn absorb_block(&mut self, block: &[u8]) {
        debug_assert_eq!(block.len() % 8, 0);

        if cfg!(target_endian = "little") {
            let state = unsafe {
                &mut *(self.state.as_mut_ptr() as *mut [u8; 8*PLEN])
            };
            for (d, i) in state.iter_mut().zip(block) {
                *d ^= *i;
            }
        } else if cfg!(target_endian = "big") {
            let n = block.len()/8;
            let mut buf = [0u64; 21];
            let buf = &mut buf[..n];
            read_u64v_le(buf, block);
            for (d, i) in self.state[..n].iter_mut().zip(buf) {
                *d ^= *i;
            }
        }

        keccak::f1600(&mut self.state);
    }

    #[inline(always)]
    pub(crate) fn as_bytes<F: FnOnce(&[u8; 8*PLEN])>(&self, f: F) {
        let mut data_copy;
        let data_ref: &[u8; 8*PLEN] = if cfg!(target_endian = "little") {
            unsafe { &*(&self.state as *const [u64; 25] as *const [u8; 8*PLEN]) }
        } else {
            data_copy = [0u8; 8*PLEN];
            write_u64v_le(&mut data_copy, &self.state);
            &data_copy
        };
        f(data_ref);
    }

    #[inline(always)]
    pub(crate) fn apply_f(&mut self) {
        keccak::f1600(&mut self.state);
    }
}
